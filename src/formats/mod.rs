//! Byte-level container parsers. Each module exposes `is_<fmt>` and `clean`.

pub mod jpeg;
pub mod png;
pub mod webp;

/// Keywords written by AI image tools (A1111, Forge, ComfyUI, InvokeAI, NovelAI, ...).
/// Compared case-insensitively.
pub(crate) const AI_KEYWORDS: &[&str] = &[
    "parameters",
    "prompt",
    "workflow",
    "negative_prompt",
    "negative prompt",
    "sd-metadata",
    "invokeai_metadata",
    "invokeai_graph",
    "generation_data",
    "generation parameters",
];

/// True if a text keyword/value looks like AI generation data.
pub(crate) fn looks_like_ai_text(keyword: &str, value: &[u8]) -> bool {
    if AI_KEYWORDS.contains(&keyword.to_ascii_lowercase().as_str()) {
        return true;
    }
    let v = String::from_utf8_lossy(value).to_ascii_lowercase();
    v.contains("steps: ") && (v.contains("sampler") || v.contains("cfg scale"))
        || v.contains("\"steps\"") && v.contains("\"seed\"")
}
