use super::*;
use crate::history::{RunRecord, insert};
use crate::score::{CandidateScore, RunScore};

fn temp_db(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "melodias-dashboard-{}-{name}.db",
        std::process::id()
    ));
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
    path
}

fn temp_out(name: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("melodias-dashboard-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    path
}

fn score() -> RunScore {
    let image = CandidateScore {
        index: 1,
        file: "/tmp/image-01.png".to_string(),
        seed: Some(7),
        selected: true,
        mode: "image",
        score: 58.25,
        points: 29.13,
        evaluated: 50.0,
        findings: Vec::new(),
        skipped: Vec::new(),
    };
    RunScore {
        rubric: crate::score::RUBRIC,
        dir: "/tmp/run".to_string(),
        technical: 72.35,
        editorial: Some(67.0),
        total: 70.21,
        accepted: false,
        gates: Vec::new(),
        images: vec![image.clone()],
        audios: Vec::new(),
        final_image: image,
        final_audio: CandidateScore {
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
        },
    }
}

fn body_bytes(response: Response) -> Vec<u8> {
    match response.body {
        Body::Bytes(bytes) => bytes,
        Body::File { .. } => panic!("expected an in-memory body"),
    }
}

#[test]
fn parses_the_request_line() {
    assert_eq!(
        request_target("GET /api/summary?limit=5 HTTP/1.1\r\n"),
        Some(("GET", "/api/summary?limit=5"))
    );
    assert_eq!(request_target("nonsense"), None);
    assert_eq!(query_limit("limit=5"), Some(5));
    assert_eq!(query_limit("x=1&limit=12"), Some(12));
    assert_eq!(query_limit("limit=abc"), None);
    assert_eq!(query_id("id=7"), Some(7));
    assert_eq!(query_id("id=nope"), None);
}

#[test]
fn parses_byte_ranges() {
    assert_eq!(parse_range("bytes=0-9", 100), Some((0, 9)));
    assert_eq!(parse_range("bytes=90-", 100), Some((90, 99)));
    assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
    assert_eq!(parse_range("bytes=95-200", 100), Some((95, 99)));
    assert_eq!(parse_range("bytes=100-", 100), None);
    assert_eq!(parse_range("bytes=9-3", 100), None);
    assert_eq!(parse_range("bytes=0-1,4-5", 100), None);
    assert_eq!(parse_range("lines=0-9", 100), None);
    assert_eq!(parse_range("bytes=0-9", 0), None);
}

#[test]
fn rejects_unsafe_relative_paths() {
    assert!(safe_relative("thumbnail.png"));
    assert!(safe_relative("candidates/image-01.png"));
    assert!(!safe_relative("../meta.json"));
    assert!(!safe_relative("candidates/../../meta.json"));
    assert!(!safe_relative("/etc/passwd"));
    assert!(!safe_relative("candidates\\image-01.png"));
    assert!(!safe_relative(""));
    assert_eq!(percent_decode("image-01.png"), Some("image-01.png".into()));
    assert_eq!(percent_decode("a%20b.png"), Some("a b.png".into()));
    assert_eq!(percent_decode("a%2Fb"), Some("a/b".into()));
    assert_eq!(percent_decode("a%2"), None);
}

#[test]
fn routes_assets_and_errors() {
    let db = temp_db("routes");
    let out = temp_out("routes");
    let state = State {
        db: db.clone(),
        out: out.clone(),
    };
    let page = route("/", &state, 10, None);
    assert_eq!(page.status, 200);
    assert!(String::from_utf8_lossy(&body_bytes(page)).contains("melodias"));

    let missing = route("/nope", &state, 10, None);
    assert_eq!(missing.status, 404);

    let json = route("/api/summary", &state, 10, None);
    assert_eq!(json.status, 200);
    let parsed: Summary = serde_json::from_slice(&body_bytes(json)).unwrap();
    assert!(!parsed.exists);
    assert!(parsed.runs.is_empty());
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn summarizes_a_database() {
    let db = temp_db("summary");
    let out = temp_out("summary");
    let dir = out.join("epic-8-bit");
    std::fs::create_dir_all(&dir).unwrap();
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
    let mut run = score();
    run.accepted = true;
    insert(&record, &run).unwrap();
    let limited = RunRecord {
        outcome: "accepted_by_limit",
        outcome_reason: Some("the image budget is exhausted at 84.2"),
        started_at: "2026-11-20T14:00:00Z",
        seconds: 1000.0,
        ..record
    };
    insert(&limited, &score()).unwrap();

    let summary = summary(&db, 20).unwrap();
    assert!(summary.exists);
    assert_eq!(summary.totals.runs, 2);
    assert_eq!(summary.totals.accepted, 1);
    assert_eq!(summary.totals.by_limit, 1);
    assert_eq!(summary.runs.len(), 2);
    assert_eq!(
        summary.runs[0].outcome.as_deref(),
        Some("accepted_by_limit")
    );
    assert_eq!(summary.runs[1].outcome.as_deref(), Some("accepted"));
    assert_eq!(summary.runs[0].slug, "epic-8-bit");
    assert_eq!(summary.rubrics, vec![crate::score::RUBRIC]);
    assert_eq!(summary.totals.findings, 0);
    assert_eq!(summary.totals.candidates, 2);
    assert_eq!(summary.totals.patches, 0);
    assert_eq!(summary.totals.lost_points, 0.0);

    let state = State {
        db: db.clone(),
        out: out.clone(),
    };
    let body = body_bytes(route("/api/summary?limit=1", &state, 20, None));
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["limit"], 1);
    assert_eq!(parsed["runs"][0]["technical"], 72.35);

    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", db.display()));
    }
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn serves_the_detail_and_the_media_of_a_run() {
    let db = temp_db("detail");
    let out = temp_out("detail");
    let dir = out.join("nested").join("epic-8-bit");
    std::fs::create_dir_all(dir.join("work").join("judge")).unwrap();
    std::fs::write(
        dir.join("meta.json"),
        br#"{"slug":"epic-8-bit","vibe":"epic","image":{"selected":1}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("work").join("judge").join("verdict.json"),
        br#"{"omni":{"best":{"score":67}}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("work").join("score.json"),
        br#"{"technical":72.35}"#,
    )
    .unwrap();
    std::fs::write(dir.join("thumbnail.png"), b"\x89PNG\r\n\x1a\n0123456789").unwrap();

    let record = RunRecord {
        db: &db,
        dir: &dir,
        slug: "epic-8-bit",
        vibe: "epic",
        started_at: "2026-11-20T13:40:44Z",
        seconds: 2123.0,
        rounds: 1,
        patches: &[],
        outcome: "accepted",
        outcome_reason: None,
    };
    insert(&record, &score()).unwrap();

    let state = State {
        db: db.clone(),
        out: out.clone(),
    };
    let response = route("/api/run?id=1", &state, 10, None);
    assert_eq!(response.status, 200);
    let detail: RunDetail = serde_json::from_slice(&body_bytes(response)).unwrap();
    assert_eq!(detail.slug, "epic-8-bit");
    assert_eq!(detail.meta.as_ref().unwrap()["vibe"], "epic");
    assert_eq!(
        detail.verdict.as_ref().unwrap()["omni"]["best"]["score"],
        67
    );
    assert_eq!(detail.score.as_ref().unwrap()["technical"], 72.35);
    assert!(
        detail
            .artifacts
            .iter()
            .any(|item| item.path == "thumbnail.png")
    );
    assert!(
        detail
            .artifacts
            .iter()
            .any(|item| item.path == "work/judge/verdict.json")
    );

    let missing = route("/api/run?id=99", &state, 10, None);
    assert_eq!(missing.status, 404);
    assert_eq!(route("/api/run", &state, 10, None).status, 400);

    let image = media(&state, "1/thumbnail.png", None);
    assert_eq!(image.status, 200);
    assert_eq!(image.content_type, "image/png");
    assert!(image.accept_ranges);

    let partial = media(&state, "1/thumbnail.png", Some("bytes=0-3"));
    assert_eq!(partial.status, 206);
    assert_eq!(partial.content_range.as_deref(), Some("bytes 0-3/18"));
    assert_eq!(partial.body.len(), 4);

    let invalid = media(&state, "1/thumbnail.png", Some("bytes=99-"));
    assert_eq!(invalid.status, 416);

    let traversal = media(&state, "1/../meta.json", None);
    assert_eq!(traversal.status, 400);

    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", db.display()));
    }
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn client_disconnects_stay_silent() {
    use std::io::ErrorKind;
    for kind in [
        ErrorKind::ConnectionReset,
        ErrorKind::ConnectionAborted,
        ErrorKind::BrokenPipe,
        ErrorKind::UnexpectedEof,
    ] {
        assert!(is_client_disconnect(&std::io::Error::new(kind, "gone")));
    }
    assert!(!is_client_disconnect(&std::io::Error::new(
        ErrorKind::TimedOut,
        "slow"
    )));
}
