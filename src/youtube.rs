//! YouTube title and description for a run. The LLM writes the title, the
//! opening and the hashtags while llama-server is loaded for the thumbnail
//! prompt; the rest is a fixed template so the channel copy stays consistent
//! between runs.

use serde::{Deserialize, Deserializer, Serialize};

/// YouTube rejects titles over 100 characters.
const TITLE_MAX: usize = 100;
const TITLE_FALLBACK: &str = "8-bit music";
const MIN_HASHTAGS: usize = 6;
const MAX_HASHTAGS: usize = 8;

const DEFAULT_HASHTAGS: [&str; 7] = [
    "#8BitMusic",
    "#GamingMusic",
    "#ChillMusic",
    "#PixelArt",
    "#RetroGaming",
    "#NostalgicMusic",
    "#Chiptune",
];

/// Shared by the local LLM and the cloud fallback (`--opencode`). The reply
/// must be one JSON object so both paths can parse it the same way.
pub const SYSTEM: &str = "You write YouTube metadata for a nostalgic 8-bit music channel. Reply \
with ONE JSON object and nothing else. Use exactly these keys: \"title\" is a short, \
evocative title of at most 70 characters, in English, with no quotes; \"intro\" is two \
short paragraphs in English for the top of the description, warm and nostalgic, with \
emojis allowed but no hashtags, links or lists; \"hashtags\" is an array of 6 to 8 \
CamelCase hashtags without spaces, each starting with #.";

pub fn user_prompt(vibe: &str) -> String {
    format!("Vibe of the track and thumbnail: {}", vibe.trim())
}

/// Tool and model labels for the fixed part of the description. They are copy,
/// not request parameters, so they can differ from `llm_model` and the server
/// aliases.
#[derive(Clone, Copy, Debug)]
pub struct Models<'a> {
    pub image: &'a str,
    pub audio: &'a str,
    pub creative: &'a str,
}

impl Default for Models<'_> {
    fn default() -> Self {
        Self {
            image: "MOSS-Image-SFT-UD-Q4_K_XL",
            audio: "ace-step",
            creative: "Qwen3-4B-Instruct-2507",
        }
    }
}

fn about(models: &Models<'_>) -> String {
    format!(
        "✨ *About this project*

This video is part of Melodías 8bit, a project exploring nostalgic gaming music, pixel art aesthetics, and AI-assisted creativity.

Some of the creative process was powered by my own local AI setup:

🖼️ *Visual generation*
stable-diffusion.cpp · {}

🎵 *Music generation*
audio.cpp · {}

🧠 *Creative assistance*
llama.cpp · {}

🛠️ *My local AI setup:*
- https://github.com/jd-apprentice/jd-llm

📂 *Project repository*
- https://github.com/melodias8bit

Thanks for listening! 💜

If you enjoyed the vibes, consider subscribing for more nostalgic 8-bit music.",
        models.image.trim(),
        audio_label(models.audio),
        models.creative.trim()
    )
}

fn audio_label(model: &str) -> String {
    let model = model.trim();
    if model.eq_ignore_ascii_case("ace-step") {
        "ACE-Step".to_string()
    } else {
        model.to_string()
    }
}

/// What the LLM returns. Every field is optional so a partial answer still
/// helps instead of throwing the whole reply away.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Draft {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub intro: String,
    #[serde(default, deserialize_with = "hashtags_or_text")]
    pub hashtags: Vec<String>,
}

/// Some models answer with a single comma-separated string instead of the array
/// the prompt asks for. Accept both shapes.
fn hashtags_or_text<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Shape {
        List(Vec<String>),
        Text(String),
    }

    Ok(match Shape::deserialize(deserializer)? {
        Shape::List(tags) => tags,
        Shape::Text(text) => text
            .split([' ', ','])
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(str::to_string)
            .collect(),
    })
}

/// The stored artifact: a title for the upload form and a description that is
/// ready to paste into YouTube.
#[derive(Clone, Debug, Serialize)]
pub struct Metadata {
    pub title: String,
    pub description: String,
    pub hashtags: Vec<String>,
}

/// Fills the missing pieces of a draft with the fixed template. It never fails:
/// a bad or absent LLM answer still leaves a usable title and description.
pub fn compose(vibe: &str, draft: Option<&Draft>, models: &Models<'_>) -> Metadata {
    let vibe = vibe.trim();
    let title = clean_title(draft.map(|draft| draft.title.as_str()).unwrap_or(""), vibe);
    let intro = draft
        .map(|draft| draft.intro.trim())
        .filter(|intro| !intro.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback_intro(vibe));
    let hashtags = clean_hashtags(
        draft
            .map(|draft| draft.hashtags.as_slice())
            .unwrap_or_default(),
    );
    let description = format!("{intro}\n\n{}\n\n{}\n", about(models), hashtags.join(" "));
    Metadata {
        title,
        description,
        hashtags,
    }
}

/// Reads a copy that is already stored in `meta.json`. `melodias youtube` uses
/// this to keep an LLM-written description instead of replacing it with the
/// static template; `--force` skips it.
pub fn from_meta(meta: &serde_json::Value) -> Option<Metadata> {
    let block = meta.get("youtube")?;
    let description = block.get("description")?.as_str()?.trim();
    if description.is_empty() {
        return None;
    }
    let title = block
        .get("title")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .unwrap_or(TITLE_FALLBACK);
    let hashtags = block
        .get("hashtags")
        .and_then(serde_json::Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Some(Metadata {
        title: title.to_string(),
        description: format!("{description}\n"),
        hashtags,
    })
}

fn clean_title(raw: &str, vibe: &str) -> String {
    let text = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = text
        .trim_matches(|character| matches!(character, '"' | '\'' | '`'))
        .trim();
    let text = if text.is_empty() {
        fallback_title(vibe)
    } else {
        text.to_string()
    };
    text.chars()
        .take(TITLE_MAX)
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// The channel publishes 8-bit music, so the fallback title says it when the
/// vibe does not. An LLM title is used as written.
fn fallback_title(vibe: &str) -> String {
    let vibe = vibe.trim();
    if vibe.is_empty() {
        return TITLE_FALLBACK.to_string();
    }
    let lower = vibe.to_ascii_lowercase();
    if lower.contains("8-bit") || lower.contains("8bit") || lower.contains("chiptune") {
        vibe.to_string()
    } else {
        format!("{vibe} · 8-bit chiptune")
    }
}

fn clean_hashtags(raw: &[String]) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for item in raw {
        let body: String = item
            .trim()
            .trim_start_matches('#')
            .chars()
            .filter(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        if body.is_empty() {
            continue;
        }
        let tag = format!("#{body}");
        if tags.iter().any(|seen| seen.eq_ignore_ascii_case(&tag)) {
            continue;
        }
        tags.push(tag);
        if tags.len() == MAX_HASHTAGS {
            break;
        }
    }
    for fallback in DEFAULT_HASHTAGS {
        if tags.len() >= MIN_HASHTAGS {
            break;
        }
        if !tags.iter().any(|seen| seen.eq_ignore_ascii_case(fallback)) {
            tags.push(fallback.to_string());
        }
    }
    tags
}

fn fallback_intro(vibe: &str) -> String {
    format!(
        "🌙 Welcome to a little corner of nostalgia.

Take a break, put on your headphones, and let this {vibe} track set the mood. 🎮

Imagine a quiet moment where pixelated worlds and 8-bit melodies meet. Whether you are gaming, studying, coding, working, or simply relaxing, let these sounds accompany your journey."
    )
}

#[cfg(test)]
#[path = "tests/youtube.rs"]
mod tests;
