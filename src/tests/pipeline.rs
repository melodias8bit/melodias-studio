use super::*;

fn finding(code: &'static str, delta: f64) -> score::Finding {
    score::Finding {
        code,
        source: "deterministic",
        severity: if delta <= -10.0 {
            "high"
        } else if delta <= -4.0 {
            "medium"
        } else {
            "low"
        },
        metric: "test",
        value: 0.0,
        threshold: 0.0,
        delta,
        message: format!("{code} {delta}"),
    }
}

fn scored(name: &str, value: Option<f64>, findings: Vec<score::Finding>) -> Scored {
    Scored {
        candidate: Candidate {
            file: PathBuf::from(name),
            seed: 1,
        },
        side: value.map(|score| score::SideScore {
            score,
            points: 0.0,
            evaluated: 50.0,
            findings,
            skipped: Vec::new(),
        }),
        prompt: String::new(),
        negative: String::new(),
        references: Vec::new(),
    }
}

#[test]
fn the_medium_is_always_in_the_audio_prompt() {
    let prompt = compose_audio_prompt("epic orchestral", "8-bit chiptune", "");
    assert!(prompt.starts_with("epic orchestral, 8-bit chiptune"));
    assert!(prompt.contains("instrumental"));
    assert!(prompt.contains("No vocals"));
    assert!(prompt.contains("TEMPO AND RHYTHM"));
    assert!(prompt.contains("STRUCTURE"));
    assert!(prompt.contains("[Theme A]"));
}

#[test]
fn the_audio_detail_is_appended_verbatim() {
    let prompt = compose_audio_prompt("epic orchestral", "8-bit chiptune", "117 BPM, lead guitars");
    assert!(prompt.contains("DETAIL\n117 BPM, lead guitars"));
    let plain = compose_audio_prompt("epic orchestral", "8-bit chiptune", "  ");
    assert!(!plain.contains("DETAIL"));
}

#[test]
fn the_best_index_skips_unscored_candidates() {
    let pool = vec![
        scored("a.png", None, Vec::new()),
        scored("b.png", Some(70.0), Vec::new()),
        scored("c.png", Some(85.0), Vec::new()),
    ];
    assert_eq!(best_index(&pool), Some(2));
    assert_eq!(best_index(&pool[..1]), None);
}

#[test]
fn the_audio_round_needs_eight_points() {
    assert!(audio_lost_enough(84.0));
    assert!(!audio_lost_enough(84.1));
    assert!(!audio_lost_enough(85.0));
    assert!(audio_lost_enough(70.0));
}

#[test]
fn the_judge_brief_keeps_measurable_losses() {
    let images = vec![scored(
        "image.png",
        Some(58.0),
        vec![
            finding("IMG_PALETTE", -20.0),
            finding("IMG_SATURATION", -2.0),
        ],
    )];
    let audios = vec![scored(
        "audio.wav",
        Some(86.0),
        vec![finding("AUD_BUILD", -5.0)],
    )];
    let brief = findings_brief(&images, &audios);
    assert!(brief.contains("image 1: IMG_PALETTE"));
    assert!(brief.contains("audio 1: AUD_BUILD"));
    assert!(!brief.contains("IMG_SATURATION"));
}
