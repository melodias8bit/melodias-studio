use super::*;

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.02,
        "expected {expected}, got {actual}"
    );
}

fn worked_image() -> ImageMetrics {
    ImageMetrics {
        width: 1280,
        height: 720,
        aspect: 16.0 / 9.0,
        unique_colors: 216_853,
        luma_std: 0.173,
        saturation: 0.378,
        glow: 0.0,
        ocr_words: Some(0),
    }
}

fn worked_audio() -> AudioMetrics {
    AudioMetrics {
        duration: 210.0,
        lufs: -14.6,
        peak: -1.4,
        centroid: 3258.0,
        high_share: 0.039_67,
        build_db: -0.232_3,
        silence: 0.0,
        leading: 0.0,
        repeat_sec: 6.0,
        variety: 1.0,
    }
}

#[test]
fn palette_buckets() {
    close(image_palette(100).points, 20.0);
    close(image_palette(256).points, 20.0);
    close(image_palette(2048).points, 10.0);
    close(image_palette(16384).points, 0.0);
    close(image_palette(216_853).points, 0.0);
}

#[test]
fn contrast_and_saturation_curves() {
    close(image_contrast(0.18).points, 10.0);
    close(image_contrast(0.10).points, 0.0);
    close(image_contrast(0.14).points, 5.0);
    close(image_contrast(0.05).points, 0.0);
    close(image_saturation(0.378).points, 5.0);
    close(image_saturation(0.80).points, 5.0 * (1.0 - 0.05 / 0.15));
    close(image_saturation(0.10).points, 0.0);
}

#[test]
fn glow_and_text_penalties() {
    close(image_glow(0.0).points, 5.0);
    close(image_glow(0.02).points, 0.0);
    close(image_glow(0.001).points, 5.0);
    close(image_text(0).points, 10.0);
    close(image_text(1).points, 0.0);
}

#[test]
fn audio_buckets() {
    close(audio_loudness(-14.6).points, 8.0);
    close(audio_loudness(-17.5).points, 0.0);
    close(audio_peak(-1.4).points, 8.0);
    close(audio_peak(-0.5).points, 4.0);
    close(audio_peak(0.5).points, 0.0);
    close(audio_centroid(3258.0).points, 8.0);
    close(audio_centroid(1000.0).points, 0.0);
    close(audio_highs(0.08).points, 8.0);
    close(audio_highs(0.02).points, 2.67);
    close(audio_build(-2.0).points, 8.0);
    close(audio_build(0.0).points, 4.0);
    close(audio_build(2.0).points, 0.0);
    close(audio_silence(0.0).points, 5.0);
    close(audio_silence(1.5).points, 2.5);
    close(audio_silence(2.5).points, 0.0);
    close(audio_duration(210.0, 210.0).points, 5.0);
    close(audio_duration(207.0, 210.0).points, 0.0);
}

#[test]
fn reproduced_worked_example() {
    let image = score_image(&worked_image());
    close(image.points, 29.13);
    close(image.score, 58.25);

    let audio = score_audio(&worked_audio(), 210.0, true);
    close(audio.points, 59.24);
    close(audio.score, 89.76);

    let technical = (image.score + audio.score) / 2.0;
    close(technical, 74.0);
    close(
        WEIGHT_TECHNICAL * technical + (1.0 - WEIGHT_TECHNICAL) * 67.0,
        71.2,
    );
}

#[test]
fn missing_ocr_renormalizes_the_image() {
    let mut metrics = worked_image();
    metrics.unique_colors = 128;
    metrics.luma_std = 0.20;
    metrics.saturation = 0.5;
    metrics.ocr_words = None;
    let score = score_image(&metrics);
    close(score.evaluated, 40.0);
    close(score.score, 100.0);
    assert_eq!(score.skipped, vec!["IMG_TEXT"]);
}

#[test]
fn drafts_skip_the_master_metrics() {
    let score = score_audio(&worked_audio(), 60.0, false);
    close(score.evaluated, 45.0);
    assert!(score.skipped.contains(&"AUD_LOUDNESS"));
    assert!(score.skipped.contains(&"AUD_PEAK"));
    assert!(score.skipped.contains(&"AUD_DURATION"));
}

#[test]
fn parses_ffmpeg_output() {
    let ebur = "  Integrated loudness:\n    I:         -14.6 LUFS\n\n  True peak:\n    Peak:       -1.4 dBFS\n";
    let (lufs, peak) = parse_ebur(ebur);
    close(lufs.unwrap(), -14.6);
    close(peak.unwrap(), -1.4);

    let centroid =
        "lavfi.aspectralstats.1.centroid=3000.0\nlavfi.aspectralstats.1.centroid=3516.0\n";
    close(parse_centroid(centroid).unwrap(), 3258.0);

    let rms = "lavfi.astats.Overall.RMS_level=-20.0\nlavfi.astats.Overall.RMS_level=-17.5\n";
    close(parse_rms(rms).unwrap(), -17.5);

    let segment = "[Parsed_astats_0 @ 0x1] Channel: 1\n[Parsed_astats_0 @ 0x1] RMS level dB: -20.0\n[Parsed_astats_0 @ 0x1] Overall\n[Parsed_astats_0 @ 0x1] RMS level dB: -17.4\n";
    close(parse_last_rms(segment).unwrap(), -17.4);

    let silence = "silence_start: 100.0\nsilence_end: 101.2 | silence_duration: 1.2\nsilence_start: 205.4\nsilence_end: 210 | silence_duration: 4.6\n";
    close(parse_silence(silence, 210.0), 1.2);
}

#[test]
fn counts_only_credible_ocr_words() {
    let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
5\t1\t1\t1\t1\t1\t0\t0\t10\t10\t90.0\tEPIC\n\
5\t1\t1\t1\t1\t2\t0\t0\t10\t10\t90.0\tab\n\
5\t1\t1\t1\t1\t3\t0\t0\t10\t10\t60.0\tSOFT\n\
5\t1\t1\t1\t1\t4\t0\t0\t10\t10\t90.0\t'-H‘\n";
    assert_eq!(count_ocr(tsv), 1);
}

#[test]
fn blank_and_text_gates_reject_the_frame() {
    let mut metrics = worked_image();
    metrics.luma_std = 0.01;
    assert!(
        image_gates(&metrics)
            .iter()
            .any(|gate| gate.code == "GATE_BLANK")
    );

    let mut metrics = worked_image();
    metrics.ocr_words = Some(1);
    assert!(image_gates(&metrics).is_empty());

    let mut metrics = worked_image();
    metrics.ocr_words = Some(3);
    assert!(
        image_gates(&metrics)
            .iter()
            .any(|gate| gate.code == "GATE_TEXT")
    );
}

#[test]
fn audio_gates_catch_clip_and_leading_silence() {
    let mut metrics = worked_audio();
    metrics.peak = 0.5;
    assert!(
        audio_gates(&metrics, 210.0)
            .iter()
            .any(|gate| gate.code == "GATE_PEAK")
    );

    let mut metrics = worked_audio();
    metrics.leading = 2.0;
    assert!(
        audio_gates(&metrics, 210.0)
            .iter()
            .any(|gate| gate.code == "GATE_LEADING")
    );

    let mut metrics = worked_audio();
    metrics.duration = 60.0;
    assert!(audio_gates(&metrics, 60.0).is_empty());
    assert!(
        audio_gates(&metrics, 210.0)
            .iter()
            .any(|gate| gate.code == "GATE_DURATION")
    );
}

#[test]
fn leading_silence_ignores_the_tail() {
    let text = "silence_start: 0.0\nsilence_end: 0.8 | silence_duration: 0.8\n\
                silence_start: 205.4\nsilence_end: 210 | silence_duration: 4.6\n";
    assert!((parse_leading(text) - 0.8).abs() < 1e-9);
    assert_eq!(
        parse_leading("silence_start: 100.0\nsilence_end: 101.2 | silence_duration: 1.2\n"),
        0.0
    );
}

#[test]
fn repeat_buckets_penalize_only_long_stalls() {
    close(audio_repeat(6.0).points, 8.0);
    close(audio_repeat(8.0).points, 8.0);
    close(audio_repeat(11.5).points, 4.0);
    close(audio_repeat(15.0).points, 0.0);
    close(audio_repeat(40.0).points, 0.0);
}

#[test]
fn longest_run_counts_similar_windows_in_seconds() {
    assert_eq!(longest_run(&[]), 0.0);
    assert_eq!(longest_run(&[(0.1, 0.05)]), 2.0);
    // Five identical windows hold one pattern for 10 s.
    let held = vec![(0.1, 0.05); 5];
    assert_eq!(longest_run(&held), 10.0);
    // A bridge in the middle splits the run.
    let mut varied = vec![(0.1, 0.05); 5];
    varied.insert(2, (0.4, 0.20));
    assert_eq!(longest_run(&varied), 6.0);
    // Silence never counts as a loop.
    let quiet = vec![(1e-4, 0.0); 10];
    assert_eq!(longest_run(&quiet), 2.0);
}

#[test]
fn window_features_see_a_held_tone() {
    // 10 s of 440 Hz then 10 s of silence: the tone holds 10 s, the tail is quiet.
    let tone: Vec<f32> = (0..8000 * 10)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 8000.0).sin() * 0.5)
        .collect();
    let mut samples = tone;
    samples.extend(std::iter::repeat_n(0.0, 8000 * 10));
    let windows = window_features(&samples);
    assert_eq!(windows.len(), 10);
    assert_eq!(longest_run(&windows), 10.0);
}

#[test]
fn fft_finds_the_test_tone() {
    // 440 Hz at 8 kHz in an 8192-point FFT lands on bin 451 (440.92 Hz).
    let mut re = vec![0.0; 8192];
    let mut im = vec![0.0; 8192];
    for (i, value) in re.iter_mut().enumerate() {
        *value = (2.0 * std::f64::consts::PI * 440.0 * i as f64 / 8000.0).sin();
    }
    fft(&mut re, &mut im);
    let peak = (1..4096)
        .max_by(|a, b| {
            let ea = re[*a] * re[*a] + im[*a] * im[*a];
            let eb = re[*b] * re[*b] + im[*b] * im[*b];
            ea.total_cmp(&eb)
        })
        .unwrap();
    assert!(peak.abs_diff(451) <= 2, "peak on bin {peak}");
}

#[test]
fn chroma_hears_a4_in_class_nine() {
    // One second of A4: pitch class 9 (69 % 12) must dominate.
    let samples: Vec<f32> = (0..8000)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 8000.0).sin() * 0.5)
        .collect();
    let windows = chroma_windows(&samples);
    assert_eq!(windows.len(), 1);
    let (chroma, rms) = &windows[0];
    assert!(*rms > 0.1);
    let top = chroma
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(index, _)| index)
        .unwrap();
    assert_eq!(top, 9);
}

fn chroma_one_hot(class: usize, rms: f64) -> ([f64; 12], f64) {
    let mut chroma = [0.0; 12];
    chroma[class] = 1.0;
    (chroma, rms)
}

#[test]
fn variety_counts_dense_windows() {
    assert_eq!(variety_share(&[]), 0.0);
    // Six distinct classes in every 30 s window: fully dense.
    let dense: Vec<_> = (0..60).map(|i| chroma_one_hot(i % 12, 0.1)).collect();
    assert_eq!(variety_share(&dense), 1.0);
    // One class everywhere: no idea ever turns over.
    let flat: Vec<_> = std::iter::repeat_n(chroma_one_hot(9, 0.1), 60).collect();
    assert_eq!(variety_share(&flat), 0.0);
    // Silence never counts as an idea.
    let quiet: Vec<_> = std::iter::repeat_n(chroma_one_hot(9, 1e-4), 30).collect();
    assert_eq!(variety_share(&quiet), 0.0);
}

#[test]
fn variety_buckets_reward_turnover() {
    close(audio_variety(0.14).points, 0.0);
    close(audio_variety(0.20).points, 0.0);
    close(audio_variety(0.40).points, 4.0);
    close(audio_variety(0.60).points, 8.0);
    close(audio_variety(1.0).points, 8.0);
}

#[test]
fn editorial_axes_weight_to_one_hundred() {
    let sum: f64 = EDITORIAL_WEIGHTS.iter().map(|(_, weight)| weight).sum();
    assert!((sum - 1.0).abs() < 1e-9);
    let axes = serde_json::json!({
        "style": 80.0,
        "composition": 70.0,
        "eyes": 90.0,
        "retro": 60.0,
        "coherence": 75.0,
    });
    // 80*.25 + 70*.25 + 90*.20 + 60*.15 + 75*.15 = 75.75
    assert!((editorial_from_axes(&axes).unwrap() - 75.75).abs() < 1e-9);
    assert!(editorial_from_axes(&serde_json::json!({"style": 80.0})).is_some());
    assert!(editorial_from_axes(&serde_json::json!({"other": 1.0})).is_none());
}
