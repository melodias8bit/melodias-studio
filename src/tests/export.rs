use super::*;
use crate::dashboard;
use crate::history::{RunRecord, insert};
use crate::score::{CandidateScore, RunScore};

const RUNS_CSV: &str = "id,started_at,slug,technical,editorial,total,accepted,outcome,outcome_reason,rounds\n\
     1,2026-11-20T13:40:44Z,epic-8-bit,72.35,67.00,70.21,true,accepted,,1\n\
     2,2026-11-21T09:00:00Z,\"neon, quest\",90.10,80.50,86.26,true,accepted_by_limit,the image budget is exhausted,2\n\
     3,2026-11-22T09:00:00Z,quiet-hymn,-,-,-,false,below_target,,0\n";

const FINDINGS_CSV: &str = "code,hits,lost,points,accepted\n\
     IMG_PALETTE,2,-20.00,-40.00,1\n\
     AUD_BUILD,1,-3.50,-3.50,0\n";

const TREND_CSV: &str = "bucket,runs,technical,total,accepted\n\
     2026-11-20,1,72.35,70.21,1\n\
     2026-11-21,2,90.10,86.26,1\n";

#[test]
fn parses_quoted_fields_and_missing_scores() {
    let runs = parse_runs_csv(RUNS_CSV).unwrap();
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[1].slug, "neon, quest");
    assert_eq!(
        runs[1].outcome_reason.as_deref(),
        Some("the image budget is exhausted")
    );
    assert_eq!(runs[2].technical, None);
    assert_eq!(runs[2].editorial, None);
    assert!(!runs[2].accepted);
    assert_eq!(outcome_label(&runs[0]), "accepted");
    assert_eq!(outcome_label(&runs[1]), "by limit");
    assert_eq!(outcome_label(&runs[2]), "below");
    assert_eq!(outcome_kind(&runs[1]), "limit");
}

#[test]
fn rejects_the_wrong_header() {
    assert!(parse_runs_csv("id,slug\n1,a\n").is_err());
    assert!(parse_findings_csv("code,hits\nIMG_PALETTE,1\n").is_err());
    assert!(parse_trend_csv("bucket,runs\n2026-11-20,1\n").is_err());
}

#[test]
fn totals_follow_the_dashboard_rules() {
    let runs = parse_runs_csv(RUNS_CSV).unwrap();
    let findings = parse_findings_csv(FINDINGS_CSV).unwrap();
    let totals = summarize(&runs, &findings);
    assert_eq!(totals.runs, 3);
    assert_eq!(totals.accepted, 2);
    assert_eq!(totals.by_limit, 1);
    assert_eq!(totals.below, 0);
    assert!((totals.technical.unwrap() - 81.225).abs() < 1e-9);
    assert!((totals.editorial.unwrap() - 73.75).abs() < 1e-9);
    assert!((totals.total.unwrap() - 78.235).abs() < 1e-9);
    assert_eq!(totals.best_slug, "neon, quest");
    assert_eq!(totals.best_id, Some(2));
    assert_eq!(totals.latest_slug, "quiet-hymn");
    assert_eq!(totals.findings, 3);
    assert!((totals.lost_points - -43.5).abs() < 1e-9);
}

#[test]
fn matches_the_dashboard_summary_for_the_same_database() {
    let db = std::env::temp_dir().join(format!("melodias-export-{}-parity.db", std::process::id()));
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", db.display()));
    }
    let dir = std::env::temp_dir().join(format!("melodias-export-{}", std::process::id()));
    let image = CandidateScore {
        index: 1,
        file: "/tmp/image-01.png".to_string(),
        seed: Some(7),
        selected: true,
        mode: "image",
        score: 58.25,
        points: 29.13,
        evaluated: 50.0,
        findings: vec![crate::score::Finding {
            code: "IMG_PALETTE",
            source: "deterministic",
            severity: "high",
            metric: "unique_colors",
            value: 216_853.0,
            threshold: 16_384.0,
            delta: -20.0,
            message: "too many colors".to_string(),
        }],
        skipped: Vec::new(),
    };
    let audio = CandidateScore {
        index: 0,
        file: "/tmp/audio.mp3".to_string(),
        seed: None,
        selected: true,
        mode: "master",
        score: 86.48,
        points: 43.24,
        evaluated: 50.0,
        findings: Vec::new(),
        skipped: Vec::new(),
    };
    let mut score = RunScore {
        rubric: crate::score::RUBRIC,
        dir: "/tmp/run".to_string(),
        technical: 72.35,
        editorial: Some(67.0),
        total: 70.21,
        accepted: true,
        gates: Vec::new(),
        images: vec![image.clone()],
        audios: Vec::new(),
        final_image: image,
        final_audio: audio,
    };
    let record = RunRecord {
        db: &db,
        dir: &dir,
        slug: "epic-8-bit",
        vibe: "epic 8-bit",
        started_at: "2026-11-20T13:40:44Z",
        seconds: 2123.0,
        rounds: 1,
        patches: &[],
        outcome: "accepted",
        outcome_reason: None,
    };
    insert(&record, &score).unwrap();
    score.accepted = false;
    let limited = RunRecord {
        outcome: "accepted_by_limit",
        outcome_reason: Some("the image budget is exhausted at 84.2"),
        started_at: "2026-11-20T14:00:00Z",
        seconds: 1000.0,
        ..record
    };
    insert(&limited, &score).unwrap();

    let summary = dashboard::summary(&db, 20).unwrap();
    let runs_csv =
        history::render_runs(&history::recent(&db, 20).unwrap(), history::Format::Csv).unwrap();
    let findings_csv =
        history::render_findings(&history::findings(&db).unwrap(), history::Format::Csv).unwrap();
    let runs = parse_runs_csv(&runs_csv).unwrap();
    let findings = parse_findings_csv(&findings_csv).unwrap();
    let totals = summarize(&runs, &findings);

    assert_eq!(totals.runs, summary.totals.runs);
    assert_eq!(totals.accepted, summary.totals.accepted);
    assert_eq!(totals.by_limit, summary.totals.by_limit);
    assert_eq!(
        totals.below,
        (summary.totals.runs - summary.totals.accepted - summary.totals.by_limit).max(0)
    );
    for (got, want) in [
        (totals.technical, summary.totals.technical),
        (totals.editorial, summary.totals.editorial),
        (totals.total, summary.totals.total),
    ] {
        assert!((got.unwrap() - want.unwrap()).abs() < 0.005);
    }
    assert_eq!(totals.findings, summary.totals.findings);
    assert!((totals.lost_points - summary.totals.lost_points).abs() < 0.05);

    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", db.display()));
    }
}

#[test]
fn renders_a_self_contained_page() {
    let runs = parse_runs_csv(RUNS_CSV).unwrap();
    let findings = parse_findings_csv(FINDINGS_CSV).unwrap();
    let trend = parse_trend_csv(TREND_CSV).unwrap();
    let page = render(&runs, &findings, &trend, "runs.csv", "2026-11-24T00:00:00Z");
    assert!(page.contains("<svg"));
    assert!(!page.contains("<script"));
    assert!(!page.contains("http://"));
    assert!(!page.contains("https://"));
    assert!(page.contains("neon, quest"));
    assert!(page.contains("quiet-hymn"));
    assert!(page.contains("target 80"));
    assert!(page.contains("IMG_PALETTE"));
    assert!(page.contains("Color palette"));
    assert!(page.contains("2026-11-21"));
    assert!(page.contains("2/3"));
    assert!(page.contains("86.3"));
    assert!(page.contains("81.2"));
    assert!(page.contains("73.8"));
    assert!(page.contains("43.5"));
}

#[test]
fn renders_an_empty_export_without_charts() {
    let page = render(&[], &[], &[], "runs.csv", "2026-11-24T00:00:00Z");
    assert!(page.contains("No runs yet."));
    assert!(!page.contains("<svg"));
}

#[test]
fn writes_index_html_into_the_out_dir() {
    let dir = std::env::temp_dir().join(format!("melodias-export-{}-out", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let runs_path = dir.join("runs.csv");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&runs_path, RUNS_CSV).unwrap();
    let out = dir.join("page");
    export_to_dir(&runs_path, None, None, &out).unwrap();
    let page = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(page.contains("epic-8-bit"));
    assert!(!page.contains("<script"));
    let _ = std::fs::remove_dir_all(&dir);
}
