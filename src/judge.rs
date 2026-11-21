use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use base64::Engine;
use serde_json::{Value, json};

use crate::config::Config;
use crate::media;
use crate::pipeline::Candidate;

const OPENROUTER_ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";
const OPENCODE_BASE: &str = "https://opencode.ai/inference/go/google/v1beta";

const SYSTEM: &str = "You are the reviewer for an 8-bit YouTube channel. The channel publishes \
original instrumental chiptune and pixel-art thumbnails. Judge only inside that brief. Return \
JSON only, with no prose and no code fences.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MustFix {
    pub code: String,
    pub target: String,
    pub reason: String,
}

pub struct Verdict {
    pub image: usize,
    pub audio: usize,
    pub score: u8,
    pub reason: String,
    pub must_fix: Vec<MustFix>,
}

impl Verdict {
    pub fn summary(&self) -> Value {
        let must_fix: Vec<Value> = self
            .must_fix
            .iter()
            .map(|item| {
                json!({
                    "code": item.code,
                    "target": item.target,
                    "reason": item.reason,
                })
            })
            .collect();
        json!({
            "image": self.image + 1,
            "audio": self.audio + 1,
            "score": self.score,
            "reason": self.reason,
            "must_fix": must_fix,
        })
    }

    /// Appends " · must fix: ..." for logs, or nothing when the judge is happy.
    pub fn must_fix_suffix(&self) -> String {
        if self.must_fix.is_empty() {
            return String::new();
        }
        let codes = self
            .must_fix
            .iter()
            .map(|item| item.code.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!(" · must fix: {codes}")
    }
}

/// Everything the judge needs. `findings` is the compact deterministic report
/// so the model does not repeat what the rubric already measured.
pub struct Request<'a> {
    pub vibe: &'a str,
    pub images: &'a [Candidate],
    pub audios: &'a [Candidate],
    pub work: &'a Path,
    pub findings: Option<&'a str>,
}

/// The two backends are not interchangeable in shape: OpenCode Go exposes
/// Gemini's native `generateContent` and takes inline bytes, while OpenRouter
/// speaks the OpenAI chat format and takes data URLs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Provider {
    OpenCode,
    OpenRouter,
}

impl Provider {
    fn label(self) -> &'static str {
        match self {
            Provider::OpenCode => "opencode",
            Provider::OpenRouter => "openrouter",
        }
    }
}

fn provider(cfg: &Config) -> Provider {
    resolve(&cfg.judge_provider, !cfg.opencode_api_key.trim().is_empty())
}

fn resolve(choice: &str, has_opencode_key: bool) -> Provider {
    match choice.trim().to_ascii_lowercase().as_str() {
        "opencode" => Provider::OpenCode,
        "openrouter" => Provider::OpenRouter,
        // Auto: prefer the subscription that is actually configured.
        _ if has_opencode_key => Provider::OpenCode,
        _ => Provider::OpenRouter,
    }
}

fn api_key(cfg: &Config) -> &str {
    match provider(cfg) {
        Provider::OpenCode => cfg.opencode_api_key.trim(),
        Provider::OpenRouter => cfg.openrouter_api_key.trim(),
    }
}

fn model(cfg: &Config) -> String {
    match provider(cfg) {
        Provider::OpenCode => cfg.judge_model.clone(),
        Provider::OpenRouter => cfg.judge_omni_model.clone(),
    }
}

pub fn enabled(cfg: &Config) -> bool {
    cfg.judge_enabled && !api_key(cfg).is_empty()
}

/// Human-readable description of the active backend, for the run log.
pub fn describe(cfg: &Config) -> String {
    format!("{} · {}", provider(cfg).label(), model(cfg))
}

/// Sends compressed evidence to the perception model, then asks the text judge
/// for the final pair. The caller decides what to do when this fails.
pub fn decide(cfg: &Config, request: Request<'_>) -> Result<Verdict> {
    let Request {
        vibe,
        images,
        audios,
        work,
        findings,
    } = request;
    let dir = work.join("judge");
    std::fs::create_dir_all(&dir).context("creating the judge directory")?;
    let evidence = evidence(cfg, images, audios, &dir)?;

    let (omni, judge, usage) = match provider(cfg) {
        Provider::OpenCode => {
            let (omni, usage) = generate(cfg, &evidence, vibe, findings)?;
            (omni, Value::Null, usage)
        }
        Provider::OpenRouter => {
            let omni = chat(
                cfg,
                &cfg.judge_omni_model,
                openrouter_content(&evidence, vibe, findings),
            )?;
            let measured = findings_section(findings);
            let judge_prompt = format!(
                "Brief: {vibe}\n\n\
                 A perception model saw the images and heard the clips. Its report:\n{omni}\n\n\
                 You are the final judge. Choose the image and the audio that make the best pair \
                 for the channel, balancing quality and mood coherence.{measured}\n\n\
                 Return exactly this JSON:\n\
                 {{\"verdict\":\"approve\",\"image\":1,\"audio\":1,\"score\":0,\
                 \"must_fix\":[{{\"code\":\"eyes\",\"target\":\"image 1\",\"reason\":\"\"}}],\
                 \"nice_to_have\":[],\"reason\":\"\"}}"
            );
            let judge = chat(cfg, &cfg.judge_reasoner_model, vec![text(judge_prompt)])
                .unwrap_or(Value::Null);
            (omni, judge, Value::Null)
        }
    };

    let image = pick_index(&judge["image"], images.len())
        .or_else(|| pick_index(&omni["best"]["image"], images.len()))
        .unwrap_or(0);
    let audio = pick_index(&judge["audio"], audios.len())
        .or_else(|| pick_index(&omni["best"]["audio"], audios.len()))
        .unwrap_or(0);
    let score = judge["score"]
        .as_u64()
        .or_else(|| omni["best"]["score"].as_u64())
        .unwrap_or(0)
        .min(100) as u8;
    let reason = judge["reason"]
        .as_str()
        .or_else(|| omni["reason"].as_str())
        .unwrap_or("no reason")
        .to_string();
    let must_fix = if judge["must_fix"].is_array() {
        parse_must_fix(&judge["must_fix"])
    } else {
        parse_must_fix(&omni["must_fix"])
    };

    std::fs::write(
        dir.join("verdict.json"),
        serde_json::to_vec_pretty(&json!({
            "provider": provider(cfg).label(),
            "model": model(cfg),
            "usage": usage,
            "omni": omni,
            "judge": judge,
            "selection": { "image": image + 1, "audio": audio + 1 },
        }))?,
    )?;

    Ok(Verdict {
        image,
        audio,
        score,
        reason,
        must_fix,
    })
}

/// The judge returns up to five fixes the deterministic rubric cannot see.
/// Objects are the current shape; a bare string is accepted from older
/// prompts. Codes are normalized and truncated so one odd answer cannot bloat
/// the report.
fn parse_must_fix(value: &Value) -> Vec<MustFix> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::new();
    let mut fixes = Vec::new();
    for item in items {
        let (code, target, reason) = if let Some(text) = item.as_str() {
            (normalize_code(text), String::new(), String::new())
        } else {
            (
                normalize_code(item["code"].as_str().unwrap_or("")),
                clean(item["target"].as_str().unwrap_or(""), 40),
                clean(item["reason"].as_str().unwrap_or(""), 200),
            )
        };
        if code.is_empty() || !seen.insert(code.clone()) {
            continue;
        }
        fixes.push(MustFix {
            code,
            target,
            reason,
        });
        if fixes.len() == 5 {
            break;
        }
    }
    fixes
}

fn normalize_code(text: &str) -> String {
    text.trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '_')
        .take(24)
        .collect()
}

fn clean(text: &str, limit: usize) -> String {
    text.trim().chars().take(limit).collect()
}

fn findings_section(findings: Option<&str>) -> String {
    match findings.map(str::trim) {
        Some(text) if !text.is_empty() => format!(
            "\n\nDeterministic findings already measured locally. Do not repeat them in must_fix:\n{text}"
        ),
        _ => String::new(),
    }
}

enum Part {
    Image { label: String, path: PathBuf },
    Audio { label: String, path: PathBuf },
}

fn evidence(
    cfg: &Config,
    images: &[Candidate],
    audios: &[Candidate],
    dir: &Path,
) -> Result<Vec<Part>> {
    let mut parts = Vec::with_capacity(images.len() + audios.len());
    for (index, candidate) in images.iter().enumerate() {
        parts.push(Part::Image {
            label: format!("Image {}", index + 1),
            path: prepare_image(cfg, candidate, dir, index)?,
        });
    }
    for (index, candidate) in audios.iter().enumerate() {
        parts.push(Part::Audio {
            label: format!("Audio {}", index + 1),
            path: prepare_audio(cfg, candidate, dir, index)?,
        });
    }
    Ok(parts)
}

fn brief(vibe: &str, images: usize, audios: usize, findings: Option<&str>) -> String {
    let measured = findings_section(findings);
    format!(
        "Brief: {vibe}\n\n\
         The channel publishes original 8-bit instrumental music and pixel-art thumbnails. \
         The candidates must be instrumental, 8-bit, 16:9, with no text or watermark. \
         You will receive {images} images and {audios} audio clips, labeled in order. \
         Score every image from 0 to 100 for adherence to the brief and channel style: \
         composition, palette, readability at thumbnail size, natural eyes, no text. \
         Score every audio clip from 0 to 100 for instrumental quality, energy, intro and \
         outro, no vocals. \
         Score the mood coherence of every image/audio pair from 0 to 100.\
         {measured}\n\n\
         must_fix is only for what the numbers cannot see. Use code one of eyes, composition, \
         style, retro, coherence or other, and target image N, audio N or pair. \
         Return exactly this JSON shape:\n\
         {{\"images\":[{{\"index\":1,\"score\":0,\"reason\":\"\"}}],\
         \"audios\":[{{\"index\":1,\"score\":0,\"reason\":\"\"}}],\
         \"pairs\":[{{\"image\":1,\"audio\":1,\"coherence\":0,\"reason\":\"\"}}],\
         \"best\":{{\"image\":1,\"audio\":1,\"score\":0}},\
         \"must_fix\":[{{\"code\":\"eyes\",\"target\":\"image 1\",\"reason\":\"\"}}],\
         \"reason\":\"\"}}"
    )
}

fn openrouter_content(parts: &[Part], vibe: &str, findings: Option<&str>) -> Vec<Value> {
    let images = parts
        .iter()
        .filter(|part| matches!(part, Part::Image { .. }))
        .count();
    let audios = parts.len() - images;
    let mut content = vec![text(brief(vibe, images, audios, findings))];
    for part in parts {
        match part {
            Part::Image { label, path } => {
                content.push(text(label.clone()));
                content.push(json!({
                    "type": "image_url",
                    "image_url": { "url": data_url(path) }
                }));
            }
            Part::Audio { label, path } => {
                content.push(text(label.clone()));
                content.push(json!({
                    "type": "input_audio",
                    "input_audio": { "data": base64_file(path), "format": "mp3" }
                }));
            }
        }
    }
    content
}

fn gemini_parts(parts: &[Part], vibe: &str, findings: Option<&str>) -> Value {
    let images = parts
        .iter()
        .filter(|part| matches!(part, Part::Image { .. }))
        .count();
    let audios = parts.len() - images;
    let mut out = vec![json!({ "text": brief(vibe, images, audios, findings) })];
    for part in parts {
        match part {
            Part::Image { label, path } => {
                out.push(json!({ "text": label }));
                out.push(json!({
                    "inline_data": {
                        "mime_type": "image/jpeg",
                        "data": base64_file(path),
                    }
                }));
            }
            Part::Audio { label, path } => {
                out.push(json!({ "text": label }));
                out.push(json!({
                    "inline_data": {
                        "mime_type": "audio/mp3",
                        "data": base64_file(path),
                    }
                }));
            }
        }
    }
    json!(out)
}

fn text(value: impl Into<String>) -> Value {
    json!({ "type": "text", "text": value.into() })
}

fn pick_index(value: &Value, count: usize) -> Option<usize> {
    let index = value.as_u64()? as usize;
    if (1..=count).contains(&index) {
        Some(index - 1)
    } else {
        None
    }
}

fn prepare_image(cfg: &Config, candidate: &Candidate, dir: &Path, index: usize) -> Result<PathBuf> {
    let path = dir.join(format!("image-{:02}.jpg", index + 1));
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-v", "error", "-y", "-i"])
        .arg(&candidate.file)
        .args([
            "-vf",
            &format!("scale={}:-2", cfg.judge_image_px.max(256)),
            "-q:v",
            "4",
        ])
        .arg(&path);
    let output = cmd.output().context("preparing the judge image")?;
    if !output.status.success() {
        bail!(
            "ffmpeg failed preparing the judge image: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(path)
}

fn prepare_audio(cfg: &Config, candidate: &Candidate, dir: &Path, index: usize) -> Result<PathBuf> {
    let duration = media::duration_secs(&candidate.file).unwrap_or(60.0);
    let start = (duration / 3.0).max(0.0);
    let seconds = cfg.judge_audio_sec.clamp(5, 30);
    let path = dir.join(format!("audio-{:02}.mp3", index + 1));
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-v", "error", "-y", "-ss"])
        .arg(format!("{start:.2}"))
        .args(["-t", &seconds.to_string(), "-i"])
        .arg(&candidate.file)
        .args(["-ac", "1", "-ar", "22050", "-b:a", "64k", "-f", "mp3"])
        .arg(&path);
    let output = cmd.output().context("preparing the judge clip")?;
    if !output.status.success() {
        bail!(
            "ffmpeg failed preparing the judge clip: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(path)
}

fn data_url(path: &Path) -> String {
    format!("data:image/jpeg;base64,{}", base64_file(path))
}

fn base64_file(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_default();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn client(cfg: &Config) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(cfg.judge_timeout_sec.max(30)))
        .build()
        .context("creating the judge client")
}

/// One-shot request to Gemini's native endpoint through OpenCode Go. The model
/// sees the images and hears the clips, so it answers the whole verdict itself.
fn generate(
    cfg: &Config,
    parts: &[Part],
    vibe: &str,
    findings: Option<&str>,
) -> Result<(Value, Value)> {
    let parsed = opencode_request(cfg, SYSTEM, gemini_parts(parts, vibe, findings))?;
    let usage = parsed.get("usageMetadata").cloned().unwrap_or(Value::Null);
    Ok((extract_json(&answer_text(&parsed)?)?, usage))
}

/// Text-only request through OpenCode Go. `melodias youtube --opencode` uses
/// it when the local LLM is off, so it only needs the API key and not the GPU.
pub fn ask_opencode(cfg: &Config, system: &str, prompt: &str) -> Result<String> {
    if cfg.opencode_api_key.trim().is_empty() {
        bail!("OPENCODE_API_KEY is not set");
    }
    let parsed = opencode_request(cfg, system, json!([{ "text": prompt }]))?;
    answer_text(&parsed)
}

fn opencode_request(cfg: &Config, system: &str, parts: Value) -> Result<Value> {
    let body = json!({
        "systemInstruction": { "parts": [{ "text": system }] },
        "contents": [{ "role": "user", "parts": parts }],
        "generationConfig": { "responseMimeType": "application/json" }
    });
    let url = format!(
        "{OPENCODE_BASE}/models/{}:generateContent",
        cfg.judge_model.trim()
    );
    let response = client(cfg)?
        .post(&url)
        .header(
            "Authorization",
            format!("Bearer {}", cfg.opencode_api_key.trim()),
        )
        .header("x-opencode-session", session())
        .header("User-Agent", "melodias-studio/0.1")
        .json(&body)
        .send()
        .context("calling OpenCode Go")?;
    let status = response.status();
    let text = response
        .text()
        .context("reading the OpenCode Go response")?;
    if !status.is_success() {
        bail!("OpenCode Go {status}: {}", truncate(&text, 500));
    }
    serde_json::from_str(&text).context("OpenCode Go returned invalid JSON")
}

fn answer_text(parsed: &Value) -> Result<String> {
    let parts = parsed
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
        .context("OpenCode Go response has no content")?;
    let answer = parts
        .iter()
        .filter(|part| part["thought"] != json!(true))
        .filter_map(|part| part["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if answer.trim().is_empty() {
        let blocked = parsed
            .pointer("/promptFeedback/blockReason")
            .and_then(Value::as_str)
            .unwrap_or("empty answer");
        bail!("OpenCode Go returned no text ({blocked})");
    }
    Ok(answer)
}

/// Stable per-process id so the gateway can route and cache the session.
fn session() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!("melodias-{:x}-{:x}", std::process::id(), nanos)
}

fn chat(cfg: &Config, model: &str, content: Vec<Value>) -> Result<Value> {
    let body = json!({
        "model": model,
        "temperature": 0.1,
        "max_tokens": 2000,
        "messages": [
            { "role": "system", "content": SYSTEM },
            { "role": "user", "content": content }
        ]
    });
    let response = client(cfg)?
        .post(OPENROUTER_ENDPOINT)
        .header(
            "Authorization",
            format!("Bearer {}", cfg.openrouter_api_key.trim()),
        )
        .header("X-Title", "melodias-studio")
        .json(&body)
        .send()
        .context("calling OpenRouter")?;
    let status = response.status();
    let body = response.text().context("reading the OpenRouter response")?;
    if !status.is_success() {
        let hint = if status.as_u16() == 402 && body.contains("audio") {
            " (audio input requires at least $0.50 in OpenRouter balance)"
        } else {
            ""
        };
        bail!("OpenRouter {status}{hint}: {}", truncate(&body, 500));
    }
    let parsed: Value = serde_json::from_str(&body).context("OpenRouter returned invalid JSON")?;
    let content = &parsed["choices"][0]["message"]["content"];
    let text = match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => bail!("OpenRouter response has no content"),
    };
    extract_json(&text)
}

fn truncate(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        text.to_string()
    } else {
        format!("{}...", &text[..limit])
    }
}

fn extract_json(text: &str) -> Result<Value> {
    let text = text.trim();
    if let Ok(value) = serde_json::from_str(text) {
        return Ok(value);
    }
    let start = text.find('{').context("no JSON object in the response")?;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (offset, character) in text[start..].char_indices() {
        if in_string {
            if escape {
                escape = false;
            } else if character == '\\' {
                escape = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let slice = &text[start..=start + offset];
                    return serde_json::from_str(slice)
                        .context("invalid JSON object in the response");
                }
            }
            _ => {}
        }
    }
    bail!("unterminated JSON object in the response")
}

#[cfg(test)]
#[path = "tests/judge.rs"]
mod tests;
