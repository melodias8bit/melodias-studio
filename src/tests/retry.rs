use super::*;

fn finding(code: &'static str, delta: f64) -> Finding {
    Finding {
        code,
        source: "deterministic",
        severity: if delta <= -10.0 { "high" } else { "medium" },
        metric: "test",
        value: 0.0,
        threshold: 0.0,
        delta,
        message: String::new(),
    }
}

#[test]
fn every_table_code_has_a_patch() {
    for code in [
        "IMG_PALETTE",
        "IMG_CONTRAST",
        "IMG_GLOW",
        "IMG_TEXT",
        "AUD_HIGHS",
        "AUD_CENTROID",
        "AUD_BUILD",
        "AUD_SILENCE",
        "AUD_REPEAT",
        "AUD_VARIETY",
    ] {
        assert!(patch_for(code).is_some(), "{code} has no patch");
    }
    assert!(patch_for("GATE_TYPE").is_none());
    assert!(patch_for("GATE_BLANK").is_none());
    assert!(patch_for("GATE_TEXT").is_none());
    assert!(patch_for("GATE_DURATION").is_none());
    assert!(patch_for("GATE_PEAK").is_none());
    assert!(patch_for("GATE_LEADING").is_none());
}

#[test]
fn chooses_the_worst_actionable_finding() {
    let findings = vec![
        finding("IMG_CONTRAST", -5.0),
        finding("IMG_PALETTE", -20.0),
        finding("IMG_SATURATION", -5.0),
    ];
    let (patch, picked) = choose(&findings, &BTreeSet::new()).unwrap();
    assert_eq!(patch.key, "IMG_PALETTE");
    assert_eq!(picked.delta, -20.0);
}

#[test]
fn ignores_low_unpatchable_and_used_codes() {
    let findings = vec![
        finding("IMG_SATURATION", -2.0),
        finding("GATE_TYPE", -20.0),
        finding("AUD_HIGHS", -8.0),
    ];
    let (patch, _) = choose(&findings, &BTreeSet::new()).unwrap();
    assert_eq!(patch.key, "AUD_TIMBRE");

    let mut used = BTreeSet::new();
    used.insert("AUD_TIMBRE");
    assert!(choose(&findings, &used).is_none());
}

#[test]
fn shared_codes_share_the_patch_key() {
    let highs = vec![finding("AUD_HIGHS", -8.0)];
    let centroid = vec![finding("AUD_CENTROID", -8.0)];
    let mut used = BTreeSet::new();
    let (patch, _) = choose(&highs, &BTreeSet::new()).unwrap();
    used.insert(patch.key);
    assert!(choose(&centroid, &used).is_none());
}

#[test]
fn applies_text_and_keeps_the_old_prompt() {
    let mut prompt = Prompt::new("pixel art castle", "text, watermark");
    let patch = patch_for("IMG_GLOW").unwrap();
    let applied = prompt.apply(patch, "IMG_GLOW");
    assert_eq!(
        prompt.positive,
        "pixel art castle, natural eyes, visible iris and pupil"
    );
    assert_eq!(prompt.negative, "text, watermark, glowing eyes, neon glow");
    let before: serde_json::Value = serde_json::from_str(&applied.prompt_before).unwrap();
    assert_eq!(before["positive"], "pixel art castle");
    assert_eq!(before["negative"], "text, watermark");
    let after: serde_json::Value = serde_json::from_str(&applied.prompt_after).unwrap();
    assert_eq!(after["positive"], prompt.positive);
    assert!(!applied.applied_at.is_empty());
}

#[test]
fn negative_only_patch_keeps_the_positive_untouched() {
    let mut prompt = Prompt::new("a castle", "glowing eyes");
    prompt.apply(patch_for("IMG_TEXT").unwrap(), "IMG_TEXT");
    assert_eq!(prompt.positive, "a castle");
    assert_eq!(
        prompt.negative,
        "glowing eyes, text, letters, watermark, signature, logo"
    );
}
