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

#[test]
fn parses_youtube_json_with_raw_controls_inside_strings() {
    let raw =
        "{\"title\":\"night\ndrive\",\"intro\":\"soft\tlullaby\",\"hashtags\":[\"#8BitMusic\"]}";
    let draft = parse_youtube(raw).unwrap();
    assert_eq!(draft.title, "night drive");
    assert_eq!(draft.intro, "soft lullaby");
    assert_eq!(draft.hashtags, vec!["#8BitMusic"]);
}

#[test]
fn sanitizer_keeps_structure_and_escapes() {
    let cleaned = sanitize_json_controls("{\"a\":\"x\\n\\\"y\"}\n{\"b\":1}");
    assert_eq!(cleaned, "{\"a\":\"x\\n\\\"y\"}\n{\"b\":1}");
}

#[test]
fn full_section_headers_make_a_valid_arrangement() {
    let text = "TEMPO AND RHYTHM\nSlow.\nLEAD\nFlute.\nRHYTHM SECTION\nSoft bass.\nLAYERS\nPads.\nHARMONY\nGentle.\nARRANGEMENT\nSparse.\nPRODUCTION\nMellow.\nSTRUCTURE\n[End]";
    assert!(valid_audio_arrangement(text));
}

#[test]
fn a_partial_arrangement_falls_back_to_the_template() {
    assert!(!valid_audio_arrangement(
        "TEMPO AND RHYTHM\nSlow.\nLEAD\nFlute."
    ));
    assert!(!valid_audio_arrangement(""));
}

#[test]
fn the_common_arrangment_typo_still_counts_as_full() {
    let text = "TEMPO AND RHYTHM\nSlow.\nLEAD\nFlute.\nRHYTHM SECTION\nSoft bass.\nLAYERS\nPads.\nHARMONY\nGentle.\nARRANGMENT\nSparse.\nPRODUCTION\nMellow.\nSTRUCTURE\n[End]";
    assert!(valid_audio_arrangement(text));
}
