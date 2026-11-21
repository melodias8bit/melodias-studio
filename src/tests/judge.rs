use super::*;

#[test]
fn extracts_json_from_fences_and_prose() {
    let text = "Here is the verdict:\n```json\n{\"image\":2,\"audio\":1,\"score\":88}\n```";
    let value = extract_json(text).unwrap();
    assert_eq!(value["image"], 2);
    assert_eq!(value["score"], 88);
}

#[test]
fn ignores_braces_inside_strings() {
    let text = "{\"reason\":\"a } inside\",\"image\":1}";
    let value = extract_json(text).unwrap();
    assert_eq!(value["image"], 1);
}

#[test]
fn accepts_only_valid_indices() {
    assert_eq!(pick_index(&json!(2), 3), Some(1));
    assert_eq!(pick_index(&json!(0), 3), None);
    assert_eq!(pick_index(&json!(4), 3), None);
}

#[test]
fn reads_opencode_text_and_skips_thoughts() {
    let parsed = json!({
        "candidates": [{
            "content": {
                "parts": [
                    { "text": "internal reasoning", "thought": true },
                    { "text": "{\"title\":\"night drive\"}" }
                ]
            }
        }]
    });
    assert_eq!(answer_text(&parsed).unwrap(), "{\"title\":\"night drive\"}");
    assert!(answer_text(&json!({})).is_err());
}

#[test]
fn parses_and_normalizes_must_fix() {
    let value = json!([
        { "code": "Eyes!", "target": "image 2", "reason": "  too much glow  " },
        "composition",
        { "code": "eyes", "target": "image 3", "reason": "duplicate" },
        { "code": "" },
        7
    ]);
    let parsed = parse_must_fix(&value);
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].code, "eyes");
    assert_eq!(parsed[0].target, "image 2");
    assert_eq!(parsed[0].reason, "too much glow");
    assert_eq!(parsed[1].code, "composition");
    assert!(parsed[1].target.is_empty());
}

#[test]
fn must_fix_caps_the_list_and_builds_the_suffix() {
    let value = json!(
        (0..9)
            .map(|index| format!("fix{index}"))
            .collect::<Vec<_>>()
    );
    let parsed = parse_must_fix(&value);
    assert_eq!(parsed.len(), 5);

    let verdict = Verdict {
        image: 0,
        audio: 0,
        score: 70,
        reason: "ok".to_string(),
        must_fix: parsed,
    };
    assert!(verdict.must_fix_suffix().starts_with(" · must fix: fix0"));
    assert!(verdict.summary()["must_fix"].is_array());
}

#[test]
fn auto_provider_prefers_the_configured_key() {
    assert_eq!(resolve("auto", false), Provider::OpenRouter);
    assert_eq!(resolve("auto", true), Provider::OpenCode);
    assert_eq!(resolve("openrouter", true), Provider::OpenRouter);
    assert_eq!(resolve("opencode", false), Provider::OpenCode);
}
