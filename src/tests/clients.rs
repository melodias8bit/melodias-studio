use super::*;

#[test]
fn glowing_eyes_are_sanitized() {
    let prompt = sanitize_eye_prompt("a girl with glowing red eyes in a forest");
    assert_eq!(prompt, "a girl with soft green eyes in a forest");
}

#[test]
fn glowing_pixel_eyes_are_sanitized() {
    let prompt = sanitize_eye_prompt(
        "Anime girl with glowing pixel eyes stands in a pixelated forest, 8-bit hues",
    );
    assert_eq!(
        prompt,
        "Anime girl with soft eyes stands in a pixelated forest, 8-bit hues"
    );
}

#[test]
fn image_extra_args_carry_seed_and_steps() {
    let prompt = with_extra_args("pixel art", 42, 12, "glowing eyes");
    assert!(prompt.starts_with("pixel art <sd_cpp_extra_args>"));
    assert!(prompt.contains("\"seed\":42"));
    assert!(prompt.contains("\"sample_steps\":12"));
    assert!(prompt.contains("\"negative_prompt\":\"glowing eyes\""));
    assert!(prompt.ends_with("</sd_cpp_extra_args>"));
}

#[test]
fn parses_youtube_json_even_with_a_fence() {
    let raw = "```json\n{\"title\":\"night drive\",\"intro\":\"Hello.\",\"hashtags\":[\"#8BitMusic\"]}\n```";
    let draft = parse_youtube(raw).unwrap();
    assert_eq!(draft.title, "night drive");
    assert_eq!(draft.intro, "Hello.");
    assert_eq!(draft.hashtags, vec!["#8BitMusic"]);
}

#[test]
fn parses_youtube_json_with_missing_keys() {
    let draft = parse_youtube("Here is your title: {\"title\": \"night drive\"} enjoy").unwrap();
    assert_eq!(draft.title, "night drive");
    assert!(draft.intro.is_empty());
    assert!(draft.hashtags.is_empty());
}

#[test]
fn parses_hashtags_given_as_one_string() {
    let draft =
        parse_youtube("{\"title\":\"night\",\"hashtags\":\"#8BitMusic, #ChillMusic #PixelArt\"}")
            .unwrap();
    assert_eq!(
        draft.hashtags,
        vec!["#8BitMusic", "#ChillMusic", "#PixelArt"]
    );
}

#[test]
fn rejects_an_empty_youtube_reply() {
    assert!(parse_youtube("no json here").is_err());
    assert!(parse_youtube("{}").is_err());
}
