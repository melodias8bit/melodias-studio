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

fn brief<'a>(vibe: &'a str, detail: &'a str) -> AudioBrief<'a> {
    AudioBrief {
        vibe,
        style: "8-bit chiptune",
        arrangement: None,
        detail,
    }
}

#[test]
fn the_medium_is_always_in_the_audio_prompt() {
    let prompt = compose_audio_prompt(brief("epic orchestral", ""));
    assert!(prompt.starts_with("epic orchestral, 8-bit chiptune"));
    assert!(prompt.contains("instrumental"));
    assert!(prompt.contains("No vocals"));
    assert!(prompt.contains("TEMPO AND RHYTHM"));
    assert!(prompt.contains("STRUCTURE"));
    assert!(prompt.contains("[Theme A]"));
}

#[test]
fn a_chill_vibe_gets_the_calm_arrangement() {
    let prompt = compose_audio_prompt(brief(
        "chill pastoral village lullaby, warm shepherd flute",
        "",
    ));
    assert!(prompt.contains("slow tempo"));
    assert!(prompt.contains("still gentle"));
    assert!(!prompt.contains("powerful kick drum patterns"));
    assert!(!prompt.contains("maximum intensity"));
}

#[test]
fn an_epic_vibe_keeps_the_driving_arrangement() {
    let prompt = compose_audio_prompt(brief("epic 8bit battle theme", ""));
    assert!(prompt.contains("powerful kick drum patterns"));
    assert!(prompt.contains("maximum intensity"));
    assert!(!prompt.contains("slow tempo"));
}

#[test]
fn an_llm_arrangement_replaces_the_keyword_template() {
    let arrangement = "TEMPO AND RHYTHM\nA haunted music box waltz in 3/4.\nLEAD\nA detuned music box.\nRHYTHM SECTION\nSparse pizzicato.\nLAYERS\nCold pads.\nHARMONY\nMinor lament.\nARRANGEMENT\nSlow build.\nPRODUCTION\nLo-fi hiss.\nSTRUCTURE\n[Intro] Box alone. [End]";
    let prompt = compose_audio_prompt(AudioBrief {
        vibe: "haunted music box waltz",
        style: "8-bit chiptune",
        arrangement: Some(arrangement),
        detail: "",
    });
    assert!(prompt.starts_with("haunted music box waltz, 8-bit chiptune"));
    assert!(prompt.contains("haunted music box waltz in 3/4"));
    assert!(!prompt.contains("powerful kick drum patterns"));
    assert!(!prompt.contains("slow tempo"));
}

#[test]
fn the_audio_detail_is_appended_verbatim() {
    let prompt = compose_audio_prompt(brief("epic orchestral", "117 BPM, lead guitars"));
    assert!(prompt.contains("DETAIL\n117 BPM, lead guitars"));
    let plain = compose_audio_prompt(brief("epic orchestral", "  "));
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
