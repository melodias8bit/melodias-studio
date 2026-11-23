//! The deterministic retry table from RFC 0002, phase 3.
//!
//! It is fixed data, not a model: a finding code selects the text that is
//! appended to the prompt before the next round. The caller owns the rounds,
//! the budget and the generation.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::score::Finding;

/// The image side allows two rounds, the audio side only one. The pipeline
/// still caps both with `SCORE_ROUNDS`.
pub const IMAGE_ROUNDS: u32 = 2;
pub const AUDIO_ROUNDS: u32 = 1;

/// A finding worth fewer points is `low` in `score-v1` and never triggers a
/// round. The RFC says "high", but the audio metrics top out at 8 points, so a
/// strict high filter would leave every audio patch unreachable.
pub const MIN_LOSS: f64 = 4.0;

#[derive(Clone, Copy, Debug)]
pub struct Patch {
    /// Stable key for the never-patch-twice rule. Codes that share one patch
    /// share the key.
    pub key: &'static str,
    pub codes: &'static [&'static str],
    pub positive: &'static str,
    pub negative: &'static str,
}

pub const PATCHES: &[Patch] = &[
    Patch {
        key: "IMG_PALETTE",
        codes: &["IMG_PALETTE"],
        positive: "limited palette, posterized, flat shading, 32 colors, pixel art",
        negative: "",
    },
    Patch {
        key: "IMG_CONTRAST",
        codes: &["IMG_CONTRAST"],
        positive: "high contrast, clear silhouette, bright rim light",
        negative: "",
    },
    Patch {
        key: "IMG_GLOW",
        codes: &["IMG_GLOW"],
        positive: "natural eyes, visible iris and pupil",
        negative: "glowing eyes, neon glow",
    },
    Patch {
        key: "IMG_TEXT",
        codes: &["IMG_TEXT"],
        positive: "",
        negative: "text, letters, watermark, signature, logo",
    },
    Patch {
        key: "AUD_TIMBRE",
        codes: &["AUD_HIGHS", "AUD_CENTROID"],
        positive: "8-bit chiptune, square and triangle waves, bitcrushed",
        negative: "",
    },
    Patch {
        key: "AUD_BUILD",
        codes: &["AUD_BUILD"],
        positive: "soft intro that builds, layered arrangement, resolved outro",
        negative: "",
    },
    Patch {
        key: "AUD_SILENCE",
        codes: &["AUD_SILENCE"],
        positive: "continuous arrangement, no long silence",
        negative: "",
    },
    Patch {
        key: "AUD_REPEAT",
        codes: &["AUD_REPEAT"],
        positive: "varied arrangement with evolving motifs, call and response, new element every 8 bars, no section repeats unchanged",
        negative: "",
    },
    Patch {
        key: "AUD_VARIETY",
        codes: &["AUD_VARIETY"],
        positive: "many distinct sections, new motifs every few bars, contrasting B section before any loop returns",
        negative: "",
    },
];

pub fn patch_for(code: &str) -> Option<&'static Patch> {
    PATCHES.iter().find(|patch| patch.codes.contains(&code))
}

/// Picks the worst actionable finding: medium or high, with a patch entry, not
/// applied before. A top finding without a patch is skipped because the loop
/// cannot fix it.
pub fn choose<'a>(
    findings: &'a [Finding],
    used: &BTreeSet<&str>,
) -> Option<(&'static Patch, &'a Finding)> {
    findings
        .iter()
        .filter(|finding| finding.delta <= -MIN_LOSS)
        .filter_map(|finding| patch_for(finding.code).map(|patch| (patch, finding)))
        .filter(|(patch, _)| !used.contains(patch.key))
        .min_by(|(_, left), (_, right)| left.delta.total_cmp(&right.delta))
}

/// The positive and negative prompt of one side, plus its applied patches.
#[derive(Clone, Debug)]
pub struct Prompt {
    pub positive: String,
    pub negative: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Applied {
    pub code: &'static str,
    pub prompt_before: String,
    pub prompt_after: String,
    pub applied_at: String,
}

impl Prompt {
    pub fn new(positive: impl Into<String>, negative: impl Into<String>) -> Self {
        Self {
            positive: positive.into(),
            negative: negative.into(),
        }
    }

    /// Appends the patch and records both prompts as JSON, because one patch
    /// can touch the positive and the negative side at once.
    pub fn apply(&mut self, patch: &Patch, code: &'static str) -> Applied {
        let before = self.snapshot();
        append(&mut self.positive, patch.positive);
        append(&mut self.negative, patch.negative);
        Applied {
            code,
            prompt_before: before,
            prompt_after: self.snapshot(),
            applied_at: crate::history::now_iso8601(),
        }
    }

    fn snapshot(&self) -> String {
        serde_json::json!({ "positive": self.positive, "negative": self.negative }).to_string()
    }
}

fn append(prompt: &mut String, text: &str) {
    if text.is_empty() {
        return;
    }
    if !prompt.trim().is_empty() {
        prompt.push_str(", ");
    }
    prompt.push_str(text);
}

#[cfg(test)]
#[path = "tests/retry.rs"]
mod tests;
