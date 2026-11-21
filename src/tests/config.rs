use super::*;

#[test]
fn reads_the_optional_fields() {
    let text = r#"
        ssh_host = "user@10.0.0.5"
        duration_sec = 240
        fade_in_sec = 2.0
        audio_negative_prompt = "vocals"
        audio_style = "lofi"
        image_style = "watercolor"
        image_model = "MOSS-Image"
        creative_model = "Qwen3-4B"
        image_negative_prompt = "glowing eyes"
        image_steps = 12
        image_seed = 42
        image_references = "/mnt/refs"
        image_reference_count = 2
        image_reference_concept = "battle"
        candidates = 3
        score_rounds = 1
        audio_draft_sec = 60
        video_renderer = "ffmpeg"
        video_particles = "sparks"
        video_project = "/tmp/video"
        pixabay_api_key = "test-key"
        audio_reference = "/tmp/loop.wav"
    "#;
    let file: FileConfig = toml::from_str(text).unwrap();
    assert_eq!(file.ssh_host.as_deref(), Some("user@10.0.0.5"));
    assert_eq!(file.duration_sec, Some(240));
    assert_eq!(file.fade_in_sec, Some(2.0));
    assert_eq!(file.audio_negative_prompt.as_deref(), Some("vocals"));
    assert_eq!(file.audio_style.as_deref(), Some("lofi"));
    assert_eq!(file.image_style.as_deref(), Some("watercolor"));
    assert_eq!(file.image_model.as_deref(), Some("MOSS-Image"));
    assert_eq!(file.creative_model.as_deref(), Some("Qwen3-4B"));
    assert_eq!(file.image_negative_prompt.as_deref(), Some("glowing eyes"));
    assert_eq!(file.image_steps, Some(12));
    assert_eq!(file.image_seed, Some(42));
    assert_eq!(file.image_references, Some(PathBuf::from("/mnt/refs")));
    assert_eq!(file.image_reference_count, Some(2));
    assert_eq!(file.image_reference_concept.as_deref(), Some("battle"));
    assert_eq!(file.candidates, Some(3));
    assert_eq!(file.score_rounds, Some(1));
    assert_eq!(file.audio_draft_sec, Some(60));
    assert_eq!(file.video_renderer.as_deref(), Some("ffmpeg"));
    assert_eq!(file.video_particles.as_deref(), Some("sparks"));
    assert_eq!(file.video_project, Some(PathBuf::from("/tmp/video")));
    assert_eq!(file.pixabay_api_key.as_deref(), Some("test-key"));
    assert_eq!(file.audio_reference, Some(PathBuf::from("/tmp/loop.wav")));
    assert_eq!(file.steps, None);
}
