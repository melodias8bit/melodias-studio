//! Local, read-only dashboard over the history database.
//!
//! It serves one page, a small JSON summary, a per-run detail (with the
//! artifacts of the run) and the media files themselves from `127.0.0.1`.
//! The page polls the summary once a minute, so a finished run shows up
//! without a reload. The server never writes to the database and never
//! creates it.

use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::history;

pub const DEFAULT_PORT: u16 = 7373;
pub const DEFAULT_LIMIT: usize = 200;

const INDEX: &str = include_str!("../dashboard/index.html");
const APP_JS: &str = include_str!("../dashboard/app.js");
const STYLE_CSS: &str = include_str!("../dashboard/style.css");

/// How deep the server looks for a run directory and its files. The normal
/// run is `output/<slug>/`, but test runs can live one level deeper, like
/// `output/retry-check/<slug>/`.
const RUN_DEPTH: u32 = 3;
const ARTIFACT_DEPTH: u32 = 3;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Totals {
    pub runs: i64,
    pub accepted: i64,
    pub by_limit: i64,
    pub cost_usd: f64,
    pub seconds: f64,
    pub technical: Option<f64>,
    pub editorial: Option<f64>,
    pub total: Option<f64>,
    pub findings: i64,
    pub lost_points: f64,
    pub candidates: i64,
    pub patches: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunPoint {
    pub id: i64,
    pub started_at: String,
    pub slug: String,
    pub vibe: String,
    pub rubric: String,
    pub technical: Option<f64>,
    pub editorial: Option<f64>,
    pub total: Option<f64>,
    pub accepted: bool,
    pub outcome: Option<String>,
    pub outcome_reason: Option<String>,
    pub cost_usd: Option<f64>,
    pub seconds: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FindingPoint {
    pub run_id: i64,
    pub code: String,
    pub source: String,
    pub severity: String,
    pub delta: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Summary {
    pub exists: bool,
    pub generated_at: String,
    pub limit: usize,
    pub totals: Totals,
    pub rubrics: Vec<String>,
    pub runs: Vec<RunPoint>,
    pub findings: Vec<FindingPoint>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CandidateDetail {
    pub id: i64,
    pub kind: String,
    pub idx: i64,
    pub seed: Option<i64>,
    pub score: Option<f64>,
    pub detail: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FindingDetail {
    pub code: String,
    pub source: String,
    pub severity: String,
    pub metric: Option<String>,
    pub value: Option<f64>,
    pub threshold: Option<f64>,
    pub delta: f64,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchDetail {
    pub code: String,
    pub prompt_before: Option<Value>,
    pub prompt_after: Option<Value>,
    pub applied_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Artifact {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub modified: Option<u64>,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunDetail {
    pub id: i64,
    pub started_at: String,
    pub slug: String,
    pub vibe: String,
    pub rubric: String,
    pub technical: Option<f64>,
    pub editorial: Option<f64>,
    pub total: Option<f64>,
    pub accepted: bool,
    pub outcome: Option<String>,
    pub outcome_reason: Option<String>,
    pub cost_usd: Option<f64>,
    pub seconds: Option<f64>,
    pub rounds: i64,
    pub config: Option<Value>,
    pub dir: Option<String>,
    pub meta: Option<Value>,
    pub verdict: Option<Value>,
    pub score: Option<Value>,
    pub candidates: Vec<CandidateDetail>,
    pub findings: Vec<FindingDetail>,
    pub patches: Vec<PatchDetail>,
    pub artifacts: Vec<Artifact>,
}

/// Serves the dashboard until the process is killed. Each request opens its
/// own read-only connection, so a run in progress is never blocked.
pub fn serve(db: PathBuf, out: PathBuf, port: u16, limit: usize) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port)).with_context(|| {
        format!("binding 127.0.0.1:{port}, is another dashboard already running?")
    })?;
    println!("dashboard: http://127.0.0.1:{port}/");
    println!("database: {}", db.display());
    println!("runs: {}", out.display());
    println!("read-only · polling the file · Ctrl+C stops it");
    let state = Arc::new(State { db, out });
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            continue;
        };
        let state = Arc::clone(&state);
        thread::spawn(move || {
            if let Err(error) = handle(stream, &state, limit) {
                eprintln!("dashboard: {error:#}");
            }
        });
    }
    Ok(())
}

struct State {
    db: PathBuf,
    out: PathBuf,
}

fn handle(mut stream: TcpStream, state: &State, default_limit: usize) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(());
    }
    let Some((method, target)) = request_target(&line) else {
        return write_response(&mut stream, Response::error(400, "bad request"), false);
    };
    // Drain the headers, keeping the only one the media route needs.
    let mut range = None;
    let mut header = String::new();
    loop {
        header.clear();
        if reader.read_line(&mut header)? == 0 || header == "\r\n" || header == "\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("range")
        {
            range = Some(value.trim().to_string());
        }
    }
    if method != "GET" && method != "HEAD" {
        return write_response(
            &mut stream,
            Response::error(405, "only GET and HEAD are allowed"),
            method == "HEAD",
        );
    }
    let response = route(target, state, default_limit, range.as_deref());
    write_response(&mut stream, response, method == "HEAD")
}

fn request_target(line: &str) -> Option<(&str, &str)> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    Some((method, target))
}

fn route(target: &str, state: &State, default_limit: usize, range: Option<&str>) -> Response {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    match path {
        "/" | "/index.html" => Response::ok("text/html; charset=utf-8", INDEX),
        "/app.js" => Response::ok("text/javascript; charset=utf-8", APP_JS),
        "/style.css" => Response::ok("text/css; charset=utf-8", STYLE_CSS),
        "/api/summary" => {
            let limit = query_limit(query).unwrap_or(default_limit).clamp(1, 1000);
            match summary(&state.db, limit).and_then(|summary| Ok(serde_json::to_vec(&summary)?)) {
                Ok(body) => Response::json(body),
                Err(error) => {
                    eprintln!("dashboard: {error:#}");
                    Response::error(
                        500,
                        "the history database could not be read; check the database path",
                    )
                }
            }
        }
        "/api/run" => {
            let Some(id) = query_id(query) else {
                return Response::error(400, "the run id is missing or invalid");
            };
            match run_detail(&state.db, &state.out, id) {
                Ok(Some(detail)) => match serde_json::to_vec(&detail) {
                    Ok(body) => Response::json(body),
                    Err(error) => {
                        eprintln!("dashboard: {error:#}");
                        Response::error(500, "the run could not be encoded")
                    }
                },
                Ok(None) => Response::error(404, "run not found"),
                Err(error) => {
                    eprintln!("dashboard: {error:#}");
                    Response::error(500, "the run could not be read")
                }
            }
        }
        path if path.starts_with("/media/") => media(state, &path["/media/".len()..], range),
        _ => Response::error(404, "not found"),
    }
}

fn query_pair<'a>(query: &'a str, wanted: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == wanted).then_some(value)
    })
}

fn query_limit(query: &str) -> Option<usize> {
    query_pair(query, "limit")?.parse().ok()
}

fn query_id(query: &str) -> Option<i64> {
    query_pair(query, "id")?.parse().ok()
}

pub fn summary(db: &Path, limit: usize) -> Result<Summary> {
    let exists = db.is_file();
    let mut summary = Summary {
        exists,
        generated_at: history::now_iso8601(),
        limit,
        totals: Totals::default(),
        rubrics: Vec::new(),
        runs: Vec::new(),
        findings: Vec::new(),
    };
    if !exists {
        return Ok(summary);
    }

    let conn = connect(db)?;
    let has_outcome = history::has_column(&conn, "runs", "outcome")?;
    let has_reason = history::has_column(&conn, "runs", "outcome_reason")?;
    let outcome_expr = if has_outcome { "outcome" } else { "null" };
    let reason_expr = if has_reason { "outcome_reason" } else { "null" };
    let by_limit = if has_outcome {
        "coalesce(sum(case when outcome = 'accepted_by_limit' then 1 else 0 end), 0)"
    } else {
        "0"
    };
    let totals_sql = format!(
        "select count(*), coalesce(sum(accepted), 0), {by_limit}, coalesce(sum(cost_usd), 0.0),
                coalesce(sum(seconds), 0.0), avg(technical), avg(editorial), avg(total),
                (select count(*) from findings),
                (select coalesce(sum(case when delta < 0 then delta else 0 end), 0.0)
                 from findings),
                (select count(*) from candidates),
                (select count(*) from patches)
         from runs"
    );
    summary.totals = conn.query_row(&totals_sql, [], |row| {
        Ok(Totals {
            runs: row.get(0)?,
            accepted: row.get(1)?,
            by_limit: row.get(2)?,
            cost_usd: row.get(3)?,
            seconds: row.get(4)?,
            technical: row.get(5)?,
            editorial: row.get(6)?,
            total: row.get(7)?,
            findings: row.get(8)?,
            lost_points: row.get(9)?,
            candidates: row.get(10)?,
            patches: row.get(11)?,
        })
    })?;

    let mut rubrics = conn.prepare("select distinct rubric from runs order by rubric")?;
    summary.rubrics = rubrics
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;

    let runs_sql = format!(
        "select id, started_at, slug, vibe, rubric, technical, editorial, total,
                accepted, cost_usd, seconds, {outcome_expr} as outcome,
                {reason_expr} as outcome_reason
         from runs order by id desc limit ?1"
    );
    let mut runs = conn.prepare(&runs_sql)?;
    summary.runs = runs
        .query_map([limit as i64], |row| {
            Ok(RunPoint {
                id: row.get(0)?,
                started_at: row.get(1)?,
                slug: row.get(2)?,
                vibe: row.get(3)?,
                rubric: row.get(4)?,
                technical: row.get(5)?,
                editorial: row.get(6)?,
                total: row.get(7)?,
                accepted: row.get(8)?,
                cost_usd: row.get(9)?,
                seconds: row.get(10)?,
                outcome: row.get(11)?,
                outcome_reason: row.get(12)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut findings = conn.prepare(
        "select run_id, code, source, severity, delta
         from findings
         where run_id in (select id from runs order by id desc limit ?1)
         order by run_id desc, code",
    )?;
    summary.findings = findings
        .query_map([limit as i64], |row| {
            Ok(FindingPoint {
                run_id: row.get(0)?,
                code: row.get(1)?,
                source: row.get(2)?,
                severity: row.get(3)?,
                delta: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(summary)
}

/// Everything the page shows for one run: the database row, its candidates,
/// findings and patches, plus the files on disk (meta.json, the judge
/// verdict, the score report and every artifact under the run directory).
pub fn run_detail(db: &Path, out: &Path, id: i64) -> Result<Option<RunDetail>> {
    if !db.is_file() {
        return Ok(None);
    }
    let conn = connect(db)?;
    let has_outcome = history::has_column(&conn, "runs", "outcome")?;
    let has_reason = history::has_column(&conn, "runs", "outcome_reason")?;
    let outcome_expr = if has_outcome { "outcome" } else { "null" };
    let reason_expr = if has_reason { "outcome_reason" } else { "null" };
    let sql = format!(
        "select id, started_at, slug, vibe, rubric, technical, editorial, total,
                accepted, cost_usd, seconds, rounds, config_json,
                {outcome_expr} as outcome, {reason_expr} as outcome_reason
         from runs where id = ?1"
    );
    let mut statement = conn.prepare(&sql)?;
    let mut rows = statement.query([id])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let slug: String = row.get(2)?;
    let mut detail = RunDetail {
        id: row.get(0)?,
        started_at: row.get(1)?,
        slug: slug.clone(),
        vibe: row.get(3)?,
        rubric: row.get(4)?,
        technical: row.get(5)?,
        editorial: row.get(6)?,
        total: row.get(7)?,
        accepted: row.get(8)?,
        cost_usd: row.get(9)?,
        seconds: row.get(10)?,
        rounds: row.get(11)?,
        config: row
            .get::<_, Option<String>>(12)?
            .and_then(|text| serde_json::from_str(&text).ok()),
        outcome: row.get(13)?,
        outcome_reason: row.get(14)?,
        dir: None,
        meta: None,
        verdict: None,
        score: None,
        candidates: Vec::new(),
        findings: Vec::new(),
        patches: Vec::new(),
        artifacts: Vec::new(),
    };

    detail.candidates = conn
        .prepare(
            "select id, kind, idx, seed, score, score_json
             from candidates where run_id = ?1 order by kind, idx",
        )?
        .query_map([id], |row| {
            let raw: Option<String> = row.get(5)?;
            Ok(CandidateDetail {
                id: row.get(0)?,
                kind: row.get(1)?,
                idx: row.get(2)?,
                seed: row.get(3)?,
                score: row.get(4)?,
                detail: raw.and_then(|text| serde_json::from_str(&text).ok()),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    detail.findings = conn
        .prepare(
            "select code, source, severity, metric, value, threshold, delta, message
             from findings where run_id = ?1 order by delta, code",
        )?
        .query_map([id], |row| {
            Ok(FindingDetail {
                code: row.get(0)?,
                source: row.get(1)?,
                severity: row.get(2)?,
                metric: row.get(3)?,
                value: row.get(4)?,
                threshold: row.get(5)?,
                delta: row.get(6)?,
                message: row.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    detail.patches = conn
        .prepare(
            "select code, prompt_before, prompt_after, applied_at
             from patches where run_id = ?1 order by id",
        )?
        .query_map([id], |row| {
            let before: Option<String> = row.get(1)?;
            let after: Option<String> = row.get(2)?;
            Ok(PatchDetail {
                code: row.get(0)?,
                prompt_before: before.map(|text| parse_or_string(&text)),
                prompt_after: after.map(|text| parse_or_string(&text)),
                applied_at: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if let Some(dir) = find_run_dir(out, &slug) {
        detail.meta = read_json(&dir.join("meta.json"));
        detail.verdict = read_json(&dir.join("work").join("judge").join("verdict.json"));
        detail.score = read_json(&dir.join("work").join("score.json"));
        detail.artifacts = artifacts(&dir, id);
        detail.dir = Some(dir.display().to_string());
    }

    Ok(Some(detail))
}

fn parse_or_string(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()))
}

fn read_json(path: &Path) -> Option<Value> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Finds the directory of a run by its slug. The normal layout is
/// `output/<slug>/`, but a test run can be nested (`output/retry-check/<slug>/`),
/// so the walk is bounded and deterministic.
fn find_run_dir(out: &Path, slug: &str) -> Option<PathBuf> {
    fn walk(dir: &Path, slug: &str, depth: u32) -> Option<PathBuf> {
        if depth > RUN_DEPTH {
            return None;
        }
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .ok()?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        entries.sort();
        for path in &entries {
            if path.file_name().is_some_and(|name| name == slug) && path.join("meta.json").is_file()
            {
                return Some(path.clone());
            }
        }
        entries
            .into_iter()
            .find_map(|path| walk(&path, slug, depth + 1))
    }
    walk(out, slug, 0)
}

fn artifacts(dir: &Path, run_id: i64) -> Vec<Artifact> {
    fn walk(dir: &Path, base: &Path, run_id: i64, depth: u32, rows: &mut Vec<Artifact>) {
        if depth > ARTIFACT_DEPTH {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, base, run_id, depth + 1, rows);
                continue;
            }
            let Some(kind) = artifact_kind(&path) else {
                continue;
            };
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            let Ok(relative) = path.strip_prefix(base) else {
                continue;
            };
            let relative = relative
                .components()
                .filter_map(|component| match component {
                    Component::Normal(part) => Some(encode_segment(&part.to_string_lossy())),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("/");
            rows.push(Artifact {
                path: relative.clone(),
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                kind: kind.to_string(),
                size: metadata.len(),
                modified: metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map(|age| age.as_secs()),
                url: format!("/media/{run_id}/{relative}"),
            });
        }
    }

    let mut rows = Vec::new();
    walk(dir, dir, run_id, 0, &mut rows);
    rows
}

fn artifact_kind(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
    Some(match extension.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" => "image",
        "mp3" | "wav" | "ogg" | "flac" => "audio",
        "mp4" | "webm" | "mov" => "video",
        "json" => "json",
        "txt" | "log" => "text",
        _ => return None,
    })
}

fn content_type(path: &Path) -> Option<&'static str> {
    Some(match artifact_kind(path)? {
        "image" => match path
            .extension()?
            .to_string_lossy()
            .to_ascii_lowercase()
            .as_str()
        {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "gif" => "image/gif",
            _ => return None,
        },
        "audio" => match path
            .extension()?
            .to_string_lossy()
            .to_ascii_lowercase()
            .as_str()
        {
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "ogg" => "audio/ogg",
            "flac" => "audio/flac",
            _ => return None,
        },
        "video" => match path
            .extension()?
            .to_string_lossy()
            .to_ascii_lowercase()
            .as_str()
        {
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "mov" => "video/quicktime",
            _ => return None,
        },
        "json" => "application/json; charset=utf-8",
        "text" => "text/plain; charset=utf-8",
        _ => return None,
    })
}

fn encode_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = *bytes.get(index + 1)?;
            let low = *bytes.get(index + 2)?;
            let byte = hex(high)? * 16 + hex(low)?;
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn hex(byte: u8) -> Option<u8> {
    Some(match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => return None,
    })
}

fn safe_relative(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.contains('\\')
        && !value.contains('\0')
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn media(state: &State, rest: &str, range: Option<&str>) -> Response {
    let Some((id, relative)) = rest.split_once('/') else {
        return Response::error(404, "not found");
    };
    let Ok(id) = id.parse::<i64>() else {
        return Response::error(404, "not found");
    };
    let Some(slug) = run_slug(&state.db, id) else {
        return Response::error(404, "run not found");
    };
    let Some(dir) = find_run_dir(&state.out, &slug) else {
        return Response::error(404, "run directory not found");
    };
    let Some(relative) = percent_decode(relative) else {
        return Response::error(400, "invalid path");
    };
    if !safe_relative(&relative) {
        return Response::error(400, "invalid path");
    }
    let Ok(base) = fs::canonicalize(&dir) else {
        return Response::error(404, "run directory not found");
    };
    let Ok(target) = fs::canonicalize(base.join(&relative)) else {
        return Response::error(404, "file not found");
    };
    if !target.starts_with(&base) || !target.is_file() {
        return Response::error(404, "file not found");
    }
    let Some(content_type) = content_type(&target) else {
        return Response::error(404, "unsupported file");
    };
    let Ok(metadata) = fs::metadata(&target) else {
        return Response::error(404, "file not found");
    };
    file_response(&target, metadata.len(), content_type, range)
}

fn run_slug(db: &Path, id: i64) -> Option<String> {
    let conn = connect(db).ok()?;
    conn.query_row("select slug from runs where id = ?1", [id], |row| {
        row.get(0)
    })
    .ok()
}

fn file_response(path: &Path, total: u64, content_type: &str, range: Option<&str>) -> Response {
    let (status, offset, length, content_range) = match range {
        None => (200, 0, total, None),
        Some(value) => match parse_range(value, total) {
            Some((start, end)) => (
                206,
                start,
                end - start + 1,
                Some(format!("bytes {start}-{end}/{total}")),
            ),
            None => {
                let mut response = Response::error(416, "range not satisfiable");
                response.content_range = Some(format!("bytes */{total}"));
                return response;
            }
        },
    };
    let Ok(file) = fs::File::open(path) else {
        return Response::error(404, "file not found");
    };
    Response {
        status,
        content_type: content_type.to_string(),
        body: Body::File {
            file,
            offset,
            length,
        },
        content_range,
        accept_ranges: true,
        inline: true,
        cache: "private, max-age=60",
    }
}

fn parse_range(value: &str, total: u64) -> Option<(u64, u64)> {
    let spec = value.trim().strip_prefix("bytes=")?;
    if spec.contains(',') || total == 0 {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    if start.is_empty() {
        let suffix: u64 = end.parse().ok()?;
        if suffix == 0 {
            return None;
        }
        let length = suffix.min(total);
        return Some((total - length, total - 1));
    }
    let start: u64 = start.parse().ok()?;
    if start >= total {
        return None;
    }
    let end = if end.is_empty() {
        total - 1
    } else {
        end.parse::<u64>().ok()?.min(total - 1)
    };
    (end >= start).then_some((start, end))
}

/// Read-only first. The fallback exists only for the WAL edge case where a
/// reader cannot map the shared memory file; it still runs SELECTs only, and
/// it never creates a missing database.
fn connect(db: &Path) -> Result<Connection> {
    let conn = match Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(conn) => conn,
        Err(primary) => {
            if !db.is_file() {
                return Err(primary).with_context(|| format!("opening {}", db.display()));
            }
            Connection::open(db).with_context(|| format!("opening {}", db.display()))?
        }
    };
    conn.busy_timeout(Duration::from_secs(2))?;
    Ok(conn)
}

enum Body {
    Bytes(Vec<u8>),
    File {
        file: fs::File,
        offset: u64,
        length: u64,
    },
}

impl Body {
    fn len(&self) -> u64 {
        match self {
            Self::Bytes(bytes) => bytes.len() as u64,
            Self::File { length, .. } => *length,
        }
    }
}

struct Response {
    status: u16,
    content_type: String,
    body: Body,
    content_range: Option<String>,
    accept_ranges: bool,
    inline: bool,
    cache: &'static str,
}

impl Response {
    fn ok(content_type: &'static str, body: &str) -> Self {
        Self {
            status: 200,
            content_type: content_type.to_string(),
            body: Body::Bytes(body.as_bytes().to_vec()),
            content_range: None,
            accept_ranges: false,
            inline: false,
            cache: "no-store",
        }
    }

    fn json(body: Vec<u8>) -> Self {
        Self {
            status: 200,
            content_type: "application/json".to_string(),
            body: Body::Bytes(body),
            content_range: None,
            accept_ranges: false,
            inline: false,
            cache: "no-store",
        }
    }

    fn error(status: u16, message: &str) -> Self {
        let body = serde_json::to_vec(&serde_json::json!({ "error": message }))
            .unwrap_or_else(|_| b"{\"error\":\"internal error\"}".to_vec());
        Self {
            status,
            content_type: "application/json".to_string(),
            body: Body::Bytes(body),
            content_range: None,
            accept_ranges: false,
            inline: false,
            cache: "no-store",
        }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        206 => "Partial Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        416 => "Range Not Satisfiable",
        _ => "Internal Server Error",
    }
}

fn write_response(
    stream: &mut TcpStream,
    response: Response,
    head_only: bool,
) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n\
         Cache-Control: {}\r\nContent-Security-Policy: default-src 'self'\r\n\
         Connection: close\r\n",
        response.status,
        reason(response.status),
        response.content_type,
        response.body.len(),
        response.cache
    );
    if response.accept_ranges {
        head.push_str("Accept-Ranges: bytes\r\n");
    }
    if response.inline {
        head.push_str("Content-Disposition: inline\r\n");
    }
    if let Some(content_range) = &response.content_range {
        head.push_str(&format!("Content-Range: {content_range}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    if !head_only {
        match response.body {
            Body::Bytes(bytes) => stream.write_all(&bytes)?,
            Body::File {
                mut file,
                offset,
                length,
            } => {
                file.seek(SeekFrom::Start(offset))?;
                let mut limited = file.take(length);
                std::io::copy(&mut limited, stream)?;
            }
        }
    }
    stream.flush()
}

#[cfg(test)]
#[path = "tests/dashboard.rs"]
mod tests;
