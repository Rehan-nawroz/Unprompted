//! WebP (RIFF): keeps image and colour chunks. Drops EXIF and XMP chunks, clears their
//! flags in the VP8X header, and rewrites the RIFF size. Image data is copied byte-for-byte.

use crate::{le_u32, Error, Kind, Removal, Result};

// VP8X feature flags (first byte of the payload).
const FLAG_EXIF: u8 = 0x08;
const FLAG_XMP: u8 = 0x04;

pub fn is_webp(data: &[u8]) -> bool {
    data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP"
}

pub fn clean(data: &[u8], keep: &[Kind]) -> Result<(Vec<u8>, Vec<Removal>)> {
    if !is_webp(data) {
        return Err(Error::Unsupported);
    }
    let riff_end = 8usize
        .checked_add(le_u32(data, 4) as usize)
        .ok_or(Error::Malformed("RIFF size overflow"))?;
    if riff_end > data.len() {
        return Err(Error::Malformed("RIFF size is larger than the file"));
    }
    if riff_end < 12 {
        return Err(Error::Malformed("RIFF size is too small"));
    }

    let mut chunks = Vec::with_capacity(riff_end);
    chunks.extend_from_slice(b"WEBP");
    let mut removed = Vec::new();
    let mut pos = 12usize;

    while pos < riff_end {
        if riff_end - pos < 8 {
            return Err(Error::Malformed("truncated chunk header"));
        }
        let fourcc: [u8; 4] = [data[pos], data[pos + 1], data[pos + 2], data[pos + 3]];
        let size = le_u32(data, pos + 4) as usize;
        let body_start = pos + 8;
        let body_end = body_start
            .checked_add(size)
            .ok_or(Error::Malformed("chunk size overflow"))?;
        if body_end > riff_end {
            return Err(Error::Malformed("chunk runs past end of file"));
        }
        let padded_end = (body_end + (size & 1)).min(riff_end);
        let body = &data[body_start..body_end];

        match &fourcc {
            b"EXIF" | b"XMP " => {
                let kind = if &fourcc == b"EXIF" { Kind::Exif } else { Kind::Xmp };
                if keep.contains(&kind) {
                    write_chunk(&mut chunks, &fourcc, body);
                } else {
                    removed.push(Removal {
                        kind,
                        name: if kind == Kind::Exif { "EXIF chunk".into() } else { "XMP chunk".into() },
                        size: padded_end - pos,
                    });
                }
            }
            b"VP8X" | b"VP8 " | b"VP8L" | b"ALPH" | b"ANIM" | b"ANMF" | b"ICCP" => {
                let mut body_copy = body.to_vec();
                if &fourcc == b"VP8X" && !body_copy.is_empty() {
                    let mut flags = body_copy[0];
                    if !keep.contains(&Kind::Exif) {
                        flags &= !FLAG_EXIF;
                    }
                    if !keep.contains(&Kind::Xmp) {
                        flags &= !FLAG_XMP;
                    }
                    body_copy[0] = flags;
                }
                write_chunk(&mut chunks, &fourcc, &body_copy);
            }
            _ => removed.push(Removal {
                kind: Kind::Other,
                name: format!("{} chunk", String::from_utf8_lossy(&fourcc)),
                size: padded_end - pos,
            }),
        }
        pos = padded_end;
    }

    if riff_end < data.len() {
        removed.push(Removal {
            kind: Kind::Other,
            name: "data after end of image".into(),
            size: data.len() - riff_end,
        });
    }

    let mut out = Vec::with_capacity(chunks.len() + 8);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
    out.extend_from_slice(&chunks);
    Ok((out, removed))
}

fn write_chunk(out: &mut Vec<u8>, fourcc: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(fourcc);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    if body.len() & 1 == 1 {
        out.push(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(fourcc: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        write_chunk(&mut v, fourcc, body);
        v
    }

    fn webp(chunks: &[Vec<u8>]) -> Vec<u8> {
        let body: Vec<u8> = chunks.concat();
        let mut v = b"RIFF".to_vec();
        v.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        v.extend_from_slice(b"WEBP");
        v.extend(body);
        v
    }

    #[test]
    fn strips_exif_and_xmp_and_clears_vp8x_flags() {
        let vp8x = [0x08 | 0x04 | 0x20 | 0x10, 0, 0, 0, 3, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        // odd-sized EXIF forces a pad byte, which must be removed with the chunk
        let input = webp(&[
            chunk(b"VP8X", &vp8x),
            chunk(b"ICCP", b"icc!"),
            chunk(b"EXIF", b"Exif-data-odd"),
            chunk(b"XMP ", b"<x:xmpmeta/>"),
            chunk(b"VP8L", &[0x2F, 0, 0, 0, 0]),
        ]);
        let (out, removed) = clean(&input, &[]).unwrap();
        let kinds: Vec<Kind> = removed.iter().map(|r| r.kind).collect();
        assert_eq!(kinds, vec![Kind::Exif, Kind::Xmp]);
        assert!(!out.windows(4).any(|w| w == b"EXIF"));
        assert!(!out.windows(4).any(|w| w == b"XMP "));
        assert!(out.windows(4).any(|w| w == b"VP8L"));
        assert!(out.windows(4).any(|w| w == b"ICCP"), "ICC profile must be kept");
        // VP8X keeps ICC and alpha, clears EXIF and XMP
        let pos = out.windows(4).position(|w| w == b"VP8X").unwrap();
        assert_eq!(out[pos + 8], 0x20 | 0x10);
        // RIFF size matches the output length
        assert_eq!(le_u32(&out, 4) as usize + 8, out.len());
        assert_eq!(out.len() + removed.iter().map(|r| r.size).sum::<usize>(), input.len());
    }

    #[test]
    fn unknown_chunks_are_removed() {
        let input = webp(&[chunk(b"VP8 ", &[1, 2, 3, 4]), chunk(b"junk", b"abc")]);
        let (out, removed) = clean(&input, &[]).unwrap();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].kind, Kind::Other);
        assert!(!out.windows(4).any(|w| w == b"junk"));
    }

    #[test]
    fn data_after_riff_is_removed() {
        let mut input = webp(&[chunk(b"VP8 ", &[1, 2, 3, 4])]);
        input.extend_from_slice(b"TAIL");
        let (out, removed) = clean(&input, &[]).unwrap();
        assert_eq!(removed.last().unwrap().size, 4);
        assert_eq!(le_u32(&out, 4) as usize + 8, out.len());
    }

    #[test]
    fn oversized_riff_is_an_error() {
        let mut input = webp(&[chunk(b"VP8 ", &[1, 2, 3, 4])]);
        let bigger = (input.len() as u32 + 100).to_le_bytes();
        input[4..8].copy_from_slice(&bigger);
        assert!(matches!(clean(&input, &[]), Err(Error::Malformed(_))));
    }

    #[test]
    fn chunk_past_riff_end_is_an_error() {
        let mut input = webp(&[chunk(b"VP8 ", &[1, 2, 3, 4])]);
        input[16..20].copy_from_slice(&100u32.to_le_bytes());
        assert!(matches!(clean(&input, &[]), Err(Error::Malformed(_))));
    }
}
