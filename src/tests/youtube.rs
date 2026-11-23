use super::*;

#[test]
fn composes_the_shared_body_and_hashtags() {
    let draft = Draft {
        title: "late night 8-bit gaming & chill".to_string(),
        intro: "Take a break and enjoy the night.".to_string(),
        hashtags: vec![
            "#8BitMusic".to_string(),
            "#GamingMusic".to_string(),
            "#ChillMusic".to_string(),
            "#PixelArt".to_string(),
        ],
    };
    let meta = compose("late night gaming", Some(&draft), &Models::default());
    assert_eq!(meta.title, "late night 8-bit gaming & chill");
    assert!(meta.description.starts_with("Take a break"));
    assert!(meta.description.contains("*About this project*"));
    assert!(meta.description.contains("Melodías 8bit"));
    assert!(meta.description.contains("github.com/jd-apprentice/jd-llm"));
    assert!(meta.description.contains("github.com/melodias8bit"));
    assert!(
        meta.description
            .contains("stable-diffusion.cpp · MOSS-Image-SFT-UD-Q4_K_XL")
    );
    assert!(meta.description.contains("audio.cpp · ACE-Step"));
    assert!(
        meta.description
            .contains("llama.cpp · Qwen3-4B-Instruct-2507")
    );
    assert_eq!(
        meta.hashtags,
        vec![
            "#8BitMusic",
            "#GamingMusic",
            "#ChillMusic",
            "#PixelArt",
            "#RetroGaming",
            "#NostalgicMusic"
        ]
    );
    assert!(
        meta.description.ends_with(
            "#8BitMusic #GamingMusic #ChillMusic #PixelArt #RetroGaming #NostalgicMusic\n"
        )
    );
}

#[test]
fn uses_the_configured_model_labels() {
    let models = Models {
        image: "Sana",
        audio: "musicgen",
        creative: "Qwen3.6-35B-A3B",
    };
    let meta = compose("vibe", None, &models);
    assert!(meta.description.contains("stable-diffusion.cpp · Sana"));
    assert!(meta.description.contains("audio.cpp · musicgen"));
    assert!(meta.description.contains("llama.cpp · Qwen3.6-35B-A3B"));
}

#[test]
fn builds_the_shared_user_prompt() {
    assert_eq!(
        user_prompt("  anime battle  "),
        "Vibe of the track and thumbnail: anime battle"
    );
}

#[test]
fn falls_back_to_the_vibe_and_default_tags() {
    let meta = compose("epic orchestral battle", None, &Models::default());
    assert_eq!(meta.title, "epic orchestral battle · 8-bit chiptune");
    assert!(meta.description.contains("epic orchestral battle"));
    assert!(meta.description.contains("#8BitMusic"));
    assert!(meta.hashtags.len() >= MIN_HASHTAGS);
}

#[test]
fn keeps_the_medium_out_of_the_fallback_title() {
    let meta = compose("late night 8-bit gaming", None, &Models::default());
    assert_eq!(meta.title, "late night 8-bit gaming");
    let meta = compose("epic chiptune duel", None, &Models::default());
    assert_eq!(meta.title, "epic chiptune duel");
}

#[test]
fn cleans_titles_and_hashtags() {
    let draft = Draft {
        title: "\"  a   title  \"".to_string(),
        intro: "Intro.".to_string(),
        hashtags: vec![
            "8 bit music".to_string(),
            "#pixel art".to_string(),
            "#PixelArt".to_string(),
        ],
    };
    let meta = compose("vibe", Some(&draft), &Models::default());
    assert_eq!(meta.title, "a title");
    assert_eq!(meta.hashtags[0], "#8bitmusic");
    assert_eq!(meta.hashtags[1], "#pixelart");
    assert_eq!(meta.hashtags.len(), MIN_HASHTAGS);
    assert!(!meta.hashtags.iter().any(|tag| tag == "#8BitMusic"));
}

#[test]
fn titles_stop_at_the_youtube_limit() {
    let long = "a".repeat(TITLE_MAX + 20);
    let meta = compose(
        "vibe",
        Some(&Draft {
            title: long,
            ..Draft::default()
        }),
        &Models::default(),
    );
    assert_eq!(meta.title.chars().count(), TITLE_MAX);
}

#[test]
fn reads_an_existing_copy_from_meta() {
    let meta = serde_json::json!({
        "youtube": {
            "title": "night drive",
            "description": "Already written.",
            "hashtags": ["#8BitMusic", "#ChillMusic"],
        }
    });
    let copy = from_meta(&meta).unwrap();
    assert_eq!(copy.title, "night drive");
    assert_eq!(copy.description, "Already written.\n");
    assert_eq!(copy.hashtags, vec!["#8BitMusic", "#ChillMusic"]);
}

#[test]
fn ignores_an_incomplete_existing_copy() {
    let meta = serde_json::json!({ "youtube": { "title": "night drive" } });
    assert!(from_meta(&meta).is_none());
    assert!(from_meta(&serde_json::json!({ "youtube": { "description": "   " } })).is_none());
}
