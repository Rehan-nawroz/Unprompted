//! JPEG: walks marker segments and keeps only what decoding and colour need.
//! Entropy-coded scan data is copied verbatim, so pixels are untouched.
//!
//! Kept: SOF/DHT/DQT/DRI/DNL/DAC/SOS/RST, JFIF APP0, ICC APP2, Adobe APP14.
//! Dropped: EXIF, XMP, IPTC, MPF, C2PA (APP11), COM, other APPn, JFXX thumbnails,
//! and any bytes after EOI (e.g. appended motion-photo video).

use super::looks_like_ai_text;
use crate::{Error, Kind, Removal, Result};

pub fn is_jpeg(data: &[u8]) -> bool {
    data.len() >= 3 && data[0] == 0xFF && data[1] == 0xD8 && data[2] == 0xFF
}

const SOI: u8 = 0xD8;
const EOI: u8 = 0xD9;
const SOS: u8 = 0xDA;
const COM: u8 = 0xFE;

pub fn clean(data: &[u8], keep: &[Kind]) -> Result<(Vec<u8>, Vec<Removal>)> {
    if !is_jpeg(data) {
        return Err(Error::Unsupported);
    }
    let len = data.len();
    let mut out = Vec::with_capacity(len);
    out.extend_from_slice(&[0xFF, SOI]);
    let mut removed = Vec::new();
    let mut i = 2usize;

    loop {
        if i >= len {
            return Err(Error::Malformed("unexpected end of JPEG data"));
        }
        if data[i] != 0xFF {
            return Err(Error::Malformed("expected a JPEG marker"));
        }
        // Skip fill bytes (0xFF padding before a marker).
        let mut j = i;
        while j < len && data[j] == 0xFF {
            j += 1;
        }
        if j >= len {
            return Err(Error::Malformed("unexpected end of JPEG data"));
        }
        let m = data[j];

        match m {
            EOI => {
                out.extend_from_slice(&[0xFF, EOI]);
                let trailing = len - (j + 1);
                if trailing > 0 {
                    removed.push(Removal {
                        kind: Kind::Other,
                        name: "data after end of image".into(),
                        size: trailing,
                    });
                }
                return Ok((out, removed));
            }
            0x00 => return Err(Error::Malformed("invalid marker 0xFF00")),
            SOI | 0x01 | 0xD0..=0xD7 => {
                // Standalone markers have no length field.
                out.extend_from_slice(&[0xFF, m]);
                i = j + 1;
                continue;
            }
            _ => {}
        }

        if j + 3 > len {
            return Err(Error::Malformed("truncated segment length"));
        }
        let seg_len = u16::from_be_bytes([data[j + 1], data[j + 2]]) as usize;
        if seg_len < 2 {
            return Err(Error::Malformed("invalid segment length"));
        }
        let seg_end = j + 1 + seg_len;
        if seg_end > len {
            return Err(Error::Malformed("segment runs past end of file"));
        }
        let payload = &data[j + 3..seg_end];

        match classify(m, payload) {
            Some(removal_kind) if !keep.contains(&removal_kind.0) => {
                removed.push(Removal {
                    kind: removal_kind.0,
                    name: removal_kind.1,
                    size: 2 + seg_len,
                });
            }
            _ => {
                out.extend_from_slice(&[0xFF, m]);
                out.extend_from_slice(&data[j + 1..seg_end]);
            }
        }

        i = seg_end;
        if m == SOS {
            // Copy entropy-coded data up to the next real marker.
            let mut k = seg_end;
            while k < len {
                if data[k] != 0xFF {
                    k += 1;
                    continue;
                }
                let next = if k + 1 < len { data[k + 1] } else { return Err(Error::Malformed("unexpected end of JPEG data")) };
                if next == 0x00 || (0xD0..=0xD7).contains(&next) {
                    k += 2; // stuffed byte or restart marker, part of the scan
                } else if next == 0xFF {
                    k += 1; // fill byte
                } else {
                    break;
                }
            }
            out.extend_from_slice(&data[seg_end..k]);
            i = k;
        }
    }
}

/// Returns `Some((kind, name))` if the segment must be removed, `None` to keep it.
fn classify(m: u8, payload: &[u8]) -> Option<(Kind, String)> {
    match m {
        // APP0: keep only the JFIF header. JFXX carries a thumbnail.
        0xE0 => {
            if payload.starts_with(b"JFIF\0") {
                None
            } else if payload.starts_with(b"JFXX\0") {
                Some((Kind::Thumbnail, "APP0 JFXX".into()))
            } else {
                Some((Kind::Other, "APP0".into()))
            }
        }
        // APP1: EXIF, XMP (and extended XMP).
        0xE1 => {
            if payload.starts_with(b"Exif\0\0") {
                Some((Kind::Exif, "APP1 Exif".into()))
            } else if payload.starts_with(b"http://ns.adobe.com/xap/1.0/")
                || payload.starts_with(b"http://ns.adobe.com/xmp/extension/")
            {
                Some((Kind::Xmp, "APP1 XMP".into()))
            } else {
                Some((Kind::Other, "APP1".into()))
            }
        }
        // APP2: keep the ICC colour profile. Drop MPF (multi-picture) and anything else.
        0xE2 => {
            if payload.starts_with(b"ICC_PROFILE\0") {
                None
            } else if payload.starts_with(b"MPF\0") {
                Some((Kind::Thumbnail, "APP2 MPF".into()))
            } else {
                Some((Kind::Other, "APP2".into()))
            }
        }
        // APP11: JPEG XT / JUMBF, where C2PA content credentials live.
        0xEB => {
            if contains(payload, b"jumb") || contains(payload, b"c2pa") {
                Some((Kind::C2pa, "APP11 JUMBF (C2PA)".into()))
            } else {
                Some((Kind::Other, "APP11".into()))
            }
        }
        // APP13: Photoshop / IPTC.
        0xED => Some((Kind::Iptc, "APP13 Photoshop/IPTC".into())),
        // APP14: Adobe colour transform flag. Needed to decode correctly.
        0xEE => {
            if payload.starts_with(b"Adobe") {
                None
            } else {
                Some((Kind::Other, "APP14".into()))
            }
        }
        COM => {
            let kind = if looks_like_ai_text("comment", payload) { Kind::AiData } else { Kind::Comment };
            Some((kind, "COM comment".into()))
        }
        // Image structure: SOF, DHT, DQT, DRI, DNL, DAC, JPG, DHP/EXP, SOS.
        0xC0..=0xCF | 0xDB | 0xDC | 0xDD | 0xDE | 0xDF | 0xDA => None,
        // Other APPn and JPEG extension markers (0xF0..0xFD) carry no image data.
        0xE3..=0xEF | 0xF0..=0xFD => Some((Kind::Other, format!("APP{}", m - 0xE0))),
        _ => Some((Kind::Other, format!("marker 0x{m:02X}"))),
    }
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(m: u8, payload: &[u8]) -> Vec<u8> {
        let mut v = vec![0xFF, m];
        v.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        v.extend_from_slice(payload);
        v
    }

    /// SOI + segments + SOS (with scan bytes incl. stuffed 0xFF00) + EOI.
    fn jpeg(segments: &[Vec<u8>]) -> Vec<u8> {
        let mut v = vec![0xFF, SOI];
        for s in segments {
            v.extend_from_slice(s);
        }
        v.extend(seg(0xDB, &[0; 65])); // DQT (kept)
        v.extend(seg(0xC0, &[8, 0, 1, 0, 1, 1, 1, 0x11, 0])); // SOF0 (kept)
        v.extend(seg(SOS, &[1, 1, 0, 0, 63, 0])); // SOS header
        v.extend_from_slice(&[0x12, 0xFF, 0x00, 0x34, 0xFF, 0xD0, 0x56]); // scan + RST0
        v.extend_from_slice(&[0xFF, EOI]);
        v
    }

    #[test]
    fn strips_exif_xmp_iptc_comment_and_keeps_scan() {
        let input = jpeg(&[
            seg(0xE0, b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0"),
            seg(0xE1, b"Exif\0\0MM\0*"),
            seg(0xE1, b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta/>"),
            seg(0xED, b"Photoshop 3.0\08BIM"),
            seg(COM, b"Steps: 30, Sampler: Euler a"),
            seg(0xEB, b"JP\0\0jumb c2pa"),
        ]);
        let (out, removed) = clean(&input, &[]).unwrap();
        let kinds: Vec<Kind> = removed.iter().map(|r| r.kind).collect();
        assert_eq!(
            kinds,
            vec![Kind::Exif, Kind::Xmp, Kind::Iptc, Kind::AiData, Kind::C2pa]
        );
        assert!(!contains(&out, b"Exif\0\0"));
        assert!(!contains(&out, b"xmpmeta"));
        assert!(!contains(&out, b"Photoshop"));
        assert!(!contains(&out, b"Steps"));
        assert!(contains(&out, b"JFIF\0"), "JFIF header must be kept");
        // scan data (with stuffed byte and RST) must be byte-identical
        assert!(contains(&out, &[0x12, 0xFF, 0x00, 0x34, 0xFF, 0xD0, 0x56]));
        assert_eq!(out.len() + removed.iter().map(|r| r.size).sum::<usize>(), input.len());
    }

    #[test]
    fn keeps_icc_profile_and_adobe_marker() {
        let input = jpeg(&[
            seg(0xE2, b"ICC_PROFILE\0\x01\x01icc-data"),
            seg(0xEE, b"Adobe\0\x64\0\0\0\0\x01"),
        ]);
        let (out, removed) = clean(&input, &[]).unwrap();
        assert!(removed.is_empty());
        assert_eq!(out, input);
    }

    #[test]
    fn drops_bytes_after_eoi() {
        let mut input = jpeg(&[]);
        input.extend_from_slice(b"MOTION-VIDEO-PAYLOAD");
        let (out, removed) = clean(&input, &[]).unwrap();
        assert!(out.ends_with(&[0xFF, EOI]));
        assert_eq!(removed.last().unwrap().size, 20);
    }

    #[test]
    fn truncated_jpeg_is_an_error() {
        let input = jpeg(&[seg(0xE1, b"Exif\0\0data")]);
        assert!(matches!(clean(&input[..input.len() - 4], &[]), Err(Error::Malformed(_))));
        assert!(matches!(clean(&[0xFF, SOI, 0xFF, 0x00, 0x01], &[]), Err(Error::Malformed(_))));
        assert!(matches!(clean(&[0xFF, SOI, 0xFF], &[]), Err(Error::Malformed(_))));
    }

    #[test]
    fn segment_longer_than_file_is_an_error() {
        let bad = vec![0xFF, SOI, 0xFF, 0xE1, 0xFF, 0xFF, 1, 2];
        assert!(matches!(clean(&bad, &[]), Err(Error::Malformed(_))));
    }

    #[test]
    fn fill_bytes_are_skipped() {
        let input = vec![0xFF, SOI, 0xFF, 0xFF, 0xFF, 0xE1, 0, 4, b'x', b'y', 0xFF, EOI];
        let (out, removed) = clean(&input, &[]).unwrap();
        assert_eq!(out, vec![0xFF, SOI, 0xFF, EOI]);
        assert_eq!(removed[0].kind, Kind::Other);
    }
}
