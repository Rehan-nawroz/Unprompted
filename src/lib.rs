//! Unprompted core library.
//!
//! Strips metadata (AI prompts/workflows, EXIF, XMP, IPTC, comments, C2PA, timestamps, ...)
//! from PNG, JPEG and WebP files. Pixel data is copied byte-for-byte, so nothing is re-encoded
//! and image quality is untouched.
//!
//! The approach is a whitelist: only chunks/segments needed to decode and display the image
//! (and its colour profile) are kept. Everything else is removed and reported.

pub mod formats;

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Supported image containers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Png,
    Jpeg,
    WebP,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Format::Png => "PNG",
            Format::Jpeg => "JPEG",
            Format::WebP => "WebP",
        }
    }

    /// Canonical file extension for this format.
    pub fn extension(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpg",
            Format::WebP => "webp",
        }
    }

    /// Extensions that are allowed to be kept as-is for this format.
    fn accepted_extensions(self) -> &'static [&'static str] {
        match self {
            Format::Png => &["png"],
            Format::Jpeg => &["jpg", "jpeg", "jpe", "jfif"],
            Format::WebP => &["webp"],
        }
    }
}

/// Category of a removed piece of metadata. Used for grouping in the UI and reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// Generation prompt, parameters or workflow written by an AI tool.
    AiData,
    Exif,
    Xmp,
    Iptc,
    /// C2PA / JUMBF content credentials.
    C2pa,
    /// Free-form text and comments.
    Comment,
    Timestamp,
    Thumbnail,
    Other,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::AiData => "AI prompt / workflow",
            Kind::Exif => "EXIF",
            Kind::Xmp => "XMP",
            Kind::Iptc => "IPTC / Photoshop",
            Kind::C2pa => "C2PA credentials",
            Kind::Comment => "Text & comments",
            Kind::Timestamp => "Timestamps",
            Kind::Thumbnail => "Thumbnail",
            Kind::Other => "Other",
        }
    }
}

/// How aggressive the cleaning is.
///
/// * `Full` strips every kind of metadata (default).
/// * `Camera` keeps the EXIF segment (camera model, settings, dates) and drops the rest,
///   including GPS inside EXIF and all AI generation data.
/// * `PromptsOnly` removes only AI generation data, comments and C2PA credentials;
///   EXIF, XMP and IPTC are preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Full,
    Camera,
    PromptsOnly,
}

impl Profile {
    /// Kinds that must survive cleaning. Everything else is removed.
    pub fn keep(self) -> &'static [Kind] {
        match self {
            Profile::Full => &[],
            Profile::Camera => &[Kind::Exif, Kind::Timestamp],
            Profile::PromptsOnly => {
                &[Kind::Exif, Kind::Xmp, Kind::Iptc, Kind::Timestamp, Kind::Thumbnail, Kind::Comment]
            }
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Profile::Full => "Full Anonymization",
            Profile::Camera => "Keep Camera Info",
            Profile::PromptsOnly => "Prompts Only",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Profile::Full => "Strips everything: AI prompts, EXIF, GPS, XMP, IPTC, C2PA.",
            Profile::Camera => "Keeps the EXIF block (camera + dates). Everything else goes.",
            Profile::PromptsOnly => "Only AI prompts, comments and C2PA are removed.",
        }
    }

    pub fn all() -> [Profile; 3] {
        [Profile::Full, Profile::Camera, Profile::PromptsOnly]
    }
}

/// One removed metadata block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removal {
    pub kind: Kind,
    /// Human readable detail, e.g. `tEXt "parameters"` or `APP1`.
    pub name: String,
    /// Size in bytes of the removed block, including its header.
    pub size: usize,
}

/// Aggregated view of removals for one kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KindSummary {
    pub kind: Kind,
    pub count: usize,
    pub bytes: usize,
}

/// Result of cleaning an in-memory image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cleaned {
    pub format: Format,
    pub data: Vec<u8>,
    pub removed: Vec<Removal>,
}

impl Cleaned {
    /// Removals grouped by kind, ordered by kind.
    pub fn summary(&self) -> Vec<KindSummary> {
        summarize(&self.removed)
    }

    /// Total number of removed blocks.
    pub fn removed_count(&self) -> usize {
        self.removed.len()
    }
}

/// Groups removals by kind.
pub fn summarize(removed: &[Removal]) -> Vec<KindSummary> {
    let mut out: Vec<KindSummary> = Vec::new();
    let mut kinds: Vec<Kind> = removed.iter().map(|r| r.kind).collect();
    kinds.sort();
    kinds.dedup();
    for kind in kinds {
        let (count, bytes) = removed
            .iter()
            .filter(|r| r.kind == kind)
            .fold((0usize, 0usize), |(c, b), r| (c + 1, b + r.size));
        out.push(KindSummary { kind, count, bytes });
    }
    out
}

#[derive(Debug)]
pub enum Error {
    /// The bytes are not PNG, JPEG or WebP.
    Unsupported,
    /// The container is damaged or truncated. The message says what went wrong.
    Malformed(&'static str),
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unsupported => write!(f, "not a supported image (PNG, JPEG or WebP)"),
            Error::Malformed(m) => write!(f, "file looks damaged: {m}"),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Detects the image format from its magic bytes (file extensions are ignored).
pub fn detect(data: &[u8]) -> Option<Format> {
    if formats::png::is_png(data) {
        Some(Format::Png)
    } else if formats::jpeg::is_jpeg(data) {
        Some(Format::Jpeg)
    } else if formats::webp::is_webp(data) {
        Some(Format::WebP)
    } else {
        None
    }
}

/// Removes metadata from an in-memory image.
pub fn clean(data: &[u8]) -> Result<Cleaned> {
    clean_with(data, Profile::Full)
}

/// Removes metadata from an in-memory image, keeping the kinds the profile preserves.
pub fn clean_with(data: &[u8], profile: Profile) -> Result<Cleaned> {
    let format = detect(data).ok_or(Error::Unsupported)?;
    let (data, removed) = match format {
        Format::Png => formats::png::clean(data, profile.keep())?,
        Format::Jpeg => formats::jpeg::clean(data, profile.keep())?,
        Format::WebP => formats::webp::clean(data, profile.keep())?,
    };
    Ok(Cleaned { format, data, removed })
}

/// Result of cleaning a file on disk.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub source: PathBuf,
    pub output: PathBuf,
    pub original_size: u64,
    pub cleaned: Cleaned,
}

impl Outcome {
    pub fn cleaned_size(&self) -> u64 {
        self.cleaned.data.len() as u64
    }
}

/// Cleans `source` and writes `<name>_clean.<ext>` next to it. The source is never modified.
/// Never overwrites an existing file: a clash gets `_clean2`, `_clean3`, and so on.
pub fn clean_file(source: &Path) -> Result<Outcome> {
    clean_file_with(source, Profile::Full)
}

/// Same as [`clean_file`] but with a cleaning [`Profile`].
pub fn clean_file_with(source: &Path, profile: Profile) -> Result<Outcome> {
    let bytes = fs::read(source)?;
    let cleaned = clean_with(&bytes, profile)?;
    let (mut file, output) = create_output(source, cleaned.format)?;
    file.write_all(&cleaned.data)?;
    file.flush()?;
    Ok(Outcome {
        source: source.to_path_buf(),
        output,
        original_size: bytes.len() as u64,
        cleaned,
    })
}

fn create_output(source: &Path, format: Format) -> Result<(File, PathBuf)> {
    let dir = source.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".to_string());
    let ext = source
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .filter(|e| format.accepted_extensions().contains(&e.as_str()))
        .unwrap_or_else(|| format.extension().to_string());

    for n in 1..=999u32 {
        let name = if n == 1 {
            format!("{stem}_clean.{ext}")
        } else {
            format!("{stem}_clean{n}.{ext}")
        };
        let path = dir.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(Error::Io(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "too many clean copies already exist",
    )))
}

/// Expands dropped/selected paths: files are kept as-is, folders contribute their direct
/// children with a supported image extension. Result is sorted and de-duplicated.
pub fn expand_inputs(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for p in paths {
        if p.is_dir() {
            if let Ok(entries) = fs::read_dir(p) {
                for e in entries.flatten() {
                    let c = e.path();
                    if c.is_file() && is_supported_name(&c) {
                        out.push(c);
                    }
                }
            }
        } else {
            out.push(p.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn is_supported_name(p: &Path) -> bool {
    p.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .is_some_and(|e| ["png", "jpg", "jpeg", "jpe", "jfif", "webp"].contains(&e.as_str()))
}

/// Byte helpers shared by the format modules.
pub(crate) fn be_u32(d: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

pub(crate) fn le_u32(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

/// Writes a ZIP archive (stored entries, no compression) so cleaned images can be
/// exported as a single file. No external dependencies, no quality risk: the bytes of
/// each entry are stored exactly as given.
pub fn write_zip(entries: &[(String, Vec<u8>)], out: &mut impl io::Write) -> io::Result<()> {
    const LFH: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];
    const CDH: [u8; 4] = [0x50, 0x4B, 0x01, 0x02];
    const EOCD: [u8; 4] = [0x50, 0x4B, 0x05, 0x06];
    const VERSION: u16 = 20;
    const FLAGS: u16 = 0x0800; // UTF-8 file names
    const METHOD: u16 = 0; // stored

    let (dos_time, dos_date) = dos_timestamp();
    let mut central = Vec::new();
    let mut offset = 0u32;

    for (name, data) in entries {
        let crc = crc32(data);
        let name_bytes = name.as_bytes();
        if name_bytes.len() > u16::MAX as usize {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "entry name too long"));
        }
        let header_start = offset;
        let mut lfh = Vec::with_capacity(30 + name_bytes.len());
        lfh.extend_from_slice(&LFH);
        lfh.extend_from_slice(&VERSION.to_le_bytes());
        lfh.extend_from_slice(&FLAGS.to_le_bytes());
        lfh.extend_from_slice(&METHOD.to_le_bytes());
        lfh.extend_from_slice(&dos_time.to_le_bytes());
        lfh.extend_from_slice(&dos_date.to_le_bytes());
        lfh.extend_from_slice(&crc.to_le_bytes());
        lfh.extend_from_slice(&(data.len() as u32).to_le_bytes());
        lfh.extend_from_slice(&(data.len() as u32).to_le_bytes());
        lfh.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        lfh.extend_from_slice(&[0, 0]); // extra field length
        lfh.extend_from_slice(name_bytes);
        out.write_all(&lfh)?;
        out.write_all(data)?;

        let mut cd = Vec::with_capacity(46 + name_bytes.len());
        cd.extend_from_slice(&CDH);
        cd.extend_from_slice(&(VERSION | 0x0300).to_le_bytes()); // made by: Unix
        cd.extend_from_slice(&VERSION.to_le_bytes());
        cd.extend_from_slice(&FLAGS.to_le_bytes());
        cd.extend_from_slice(&METHOD.to_le_bytes());
        cd.extend_from_slice(&dos_time.to_le_bytes());
        cd.extend_from_slice(&dos_date.to_le_bytes());
        cd.extend_from_slice(&crc.to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        cd.extend_from_slice(&[0, 0, 0, 0]); // extra field + comment lengths
        cd.extend_from_slice(&[0, 0]); // disk number
        cd.extend_from_slice(&[0, 0]); // internal attrs
        cd.extend_from_slice(&[0, 0, 0, 0]); // external attrs
        cd.extend_from_slice(&header_start.to_le_bytes());
        cd.extend_from_slice(name_bytes);
        central.extend_from_slice(&cd);
        offset += (lfh.len() + data.len()) as u32;
    }

    let central_start = offset;
    out.write_all(&central)?;
    let central_len = central.len() as u32;
    let mut end = Vec::with_capacity(22);
    end.extend_from_slice(&EOCD);
    end.extend_from_slice(&[0, 0]); // disk number
    end.extend_from_slice(&[0, 0]); // central dir disk
    end.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    end.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    end.extend_from_slice(&central_len.to_le_bytes());
    end.extend_from_slice(&central_start.to_le_bytes());
    end.extend_from_slice(&[0, 0]); // comment length
    out.write_all(&end)?;
    Ok(())
}

fn crc32(data: &[u8]) -> u32 {
    // Standard CRC-32 (IEEE), computed bit by bit: small, no lookup table needed.
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Current time as MS-DOS date/time fields (UTC). Files before 1980 clamp to the epoch.
fn dos_timestamp() -> (u16, u16) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as u32;
    let rem = (secs % 86_400) as u32;
    let (hour, min, sec) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // Civil-from-days (Howard Hinnant's algorithm), epoch 1970-01-01.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u16;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u16;
    let year = if month <= 2 { year + 1 } else { year };
    let year = year.clamp(1980, 2107) as u16;
    let time = (hour as u16) << 11 | (min as u16) << 5 | (sec as u16 / 2);
    let date = (year - 1980) << 9 | month << 5 | day;
    (time, date)
}

#[cfg(test)]
mod zip_tests {
    use super::*;

    fn read_u16(d: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([d[at], d[at + 1]])
    }

    fn read_u32(d: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
    }

    #[test]
    fn zip_round_trips_and_stores_bytes_exactly() {
        let entries = vec![
            ("a_clean.png".to_owned(), vec![1u8, 2, 3, 4, 5]),
            ("b_clean.jpg".to_owned(), vec![9u8; 300]),
        ];
        let mut zip = Vec::new();
        write_zip(&entries, &mut zip).unwrap();

        // EOCD points at the central directory; walk central records and compare data.
        let eocd = zip.windows(4).rposition(|w| w == [0x50, 0x4B, 0x05, 0x06]).unwrap();
        assert_eq!(read_u16(&zip, eocd + 10), 2);
        let mut p = read_u32(&zip, eocd + 16) as usize;
        for (name, data) in &entries {
            assert_eq!(&zip[p..p + 4], &[0x50, 0x4B, 0x01, 0x02]);
            let name_len = read_u16(&zip, p + 28) as usize;
            let crc = read_u32(&zip, p + 16);
            let size = read_u32(&zip, p + 24) as usize;
            let local = read_u32(&zip, p + 42) as usize;
            assert_eq!(&zip[p + 46..p + 46 + name_len], name.as_bytes());
            assert_eq!(crc, crc32(data));
            assert_eq!(size, data.len());
            // local header name length + 30-byte fixed part
            let lname = read_u16(&zip, local + 26) as usize;
            let data_start = local + 30 + lname;
            assert_eq!(&zip[data_start..data_start + size], data);
            p += 46 + name_len;
        }
    }

    #[test]
    fn crc32_matches_known_value() {
        // CRC-32 of "123456789" is 0xCBF43926.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn profiles_keep_the_right_kinds() {
        assert!(Profile::Full.keep().is_empty());
        assert!(Profile::Camera.keep().contains(&Kind::Exif));
        assert!(!Profile::Camera.keep().contains(&Kind::AiData));
        assert!(Profile::PromptsOnly.keep().contains(&Kind::Xmp));
        assert!(!Profile::PromptsOnly.keep().contains(&Kind::C2pa));
    }

    #[test]
    fn profile_filters_cleaning() {
        // PNG with AI text + EXIF + comment: PromptsOnly keeps EXIF and comment.
        let ihdr = [0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0];
        let mut input = vec![0x89u8, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut chunk = |ty: &[u8; 4], body: &[u8]| {
            input.extend((body.len() as u32).to_be_bytes());
            input.extend_from_slice(ty);
            input.extend_from_slice(body);
            input.extend_from_slice(&[0, 0, 0, 0]);
        };
        chunk(b"IHDR", &ihdr);
        chunk(b"tEXt", b"parameters\0steps: 20");
        chunk(b"eXIf", b"MM\0*");
        chunk(b"tEXt", b"Author\0Rana");
        chunk(b"IDAT", &[0x78, 0x9C, 0x63, 0x00]);
        chunk(b"IEND", &[]);

        let cleaned = clean_with(&input, Profile::PromptsOnly).unwrap();
        assert!(cleaned.data.windows(4).any(|w| w == b"eXIf"));
        assert!(cleaned.data.windows(6).any(|w| w == b"Author"));
        assert!(!cleaned.data.windows(6).any(|w| w == b"steps:"));
        assert!(cleaned.removed.iter().any(|r| r.kind == Kind::AiData));
        assert!(!cleaned.removed.iter().any(|r| r.kind == Kind::Exif));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_formats_by_magic_bytes() {
        assert_eq!(detect(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]), Some(Format::Png));
        assert_eq!(detect(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(Format::Jpeg));
        assert_eq!(detect(b"RIFF\0\0\0\0WEBPVP8 "), Some(Format::WebP));
        assert_eq!(detect(b"GIF89a"), None);
        assert_eq!(detect(b""), None);
    }

    #[test]
    fn unsupported_input_is_an_error() {
        assert!(matches!(clean(b"hello world"), Err(Error::Unsupported)));
    }

    #[test]
    fn summarize_groups_by_kind() {
        let removed = vec![
            Removal { kind: Kind::Exif, name: "a".into(), size: 10 },
            Removal { kind: Kind::AiData, name: "b".into(), size: 5 },
            Removal { kind: Kind::Exif, name: "c".into(), size: 7 },
        ];
        let s = summarize(&removed);
        assert_eq!(
            s,
            vec![
                KindSummary { kind: Kind::AiData, count: 1, bytes: 5 },
                KindSummary { kind: Kind::Exif, count: 2, bytes: 17 },
            ]
        );
    }

    #[test]
    fn output_name_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("mr-unit-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("pic.PNG");
        let (_f1, p1) = create_output(&src, Format::Png).unwrap();
        let (_f2, p2) = create_output(&src, Format::Png).unwrap();
        assert_eq!(p1.file_name().unwrap(), "pic_clean.png");
        assert_eq!(p2.file_name().unwrap(), "pic_clean2.png");
        drop((_f1, _f2));
        fs::remove_dir_all(&dir).unwrap();
    }
}
