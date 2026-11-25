use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use base64::Engine;
use serde_json::json;

use crate::config::Config;

fn short_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .context("creating the HTTP client")
}

/// One generation on the P40 takes about six minutes for a 210 s take at 32
/// steps including the model load (measured 2026-11-25), so generation requests
/// get their own budget instead of the short one used by the health probes.
fn long_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3600))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .context("creating the HTTP client")
}

pub fn check_audio(cfg: &Config) -> Result<()> {
    let url = format!("{}/health", cfg.audio_url.trim_end_matches('/'));
    let body = short_client()?
        .get(&url)
        .send()
        .and_then(|r| r.error_for_status())
        .with_context(|| format!("audio.cpp does not answer at {url}"))?
        .text()?;
    if !body.contains("\"ok\"") {
        bail!("audio.cpp returned something unexpected: {body}");
    }
    Ok(())
}

pub fn check_llm(cfg: &Config) -> Result<()> {
    let url = format!("{}/health", cfg.llm_url.trim_end_matches('/'));
    short_client()?
        .get(&url)
        .send()
        .and_then(|r| r.error_for_status())
        .with_context(|| format!("llama-server does not answer at {url}"))?;
    Ok(())
}

pub fn check_sd(cfg: &Config) -> Result<()> {
    let url = format!("{}/v1/models", cfg.sd_url.trim_end_matches('/'));
    short_client()?
        .get(&url)
        .send()
        .and_then(|r| r.error_for_status())
        .with_context(|| format!("sd-server does not answer at {url}"))?;
    Ok(())
}

pub fn wait_audio(cfg: &Config, timeout: Duration) -> Result<()> {
    wait("audio.cpp", timeout, || check_audio(cfg))
}

pub fn wait_llm(cfg: &Config, timeout: Duration) -> Result<()> {
    wait("llama-server", timeout, || check_llm(cfg))
}

pub fn wait_sd(cfg: &Config, timeout: Duration) -> Result<()> {
    wait("sd-server", timeout, || check_sd(cfg))
}

fn wait(name: &str, timeout: Duration, probe: impl Fn() -> Result<()>) -> Result<()> {
    let start = std::time::Instant::now();
    loop {
        if probe().is_ok() {
            return Ok(());
        }
        if start.elapsed() > timeout {
            bail!("{name} did not answer within {} s", timeout.as_secs());
        }
        std::thread::sleep(Duration::from_secs(3));
    }
}

pub fn ask_image_prompt(cfg: &Config, vibe: &str) -> Result<String> {
    let url = format!("{}/v1/chat/completions", cfg.llm_url.trim_end_matches('/'));
    let system = format!(
        "You write prompts for a text-to-image model. Reply with a single paragraph of 120 to 180 \
words, in English, no quotes, no explanation, no headers. Describe a 16:9 YouTube thumbnail \
in {} style, covering in order: composition (one anime character in a scene, readable at \
small size), the character (pose, outfit, expression, natural eyes with a visible iris and \
pupil; never glowing, neon or empty eyes), the background (detailed, layered, nostalgic \
cozy mood), and palette and light (limited palette, clear key light, rim light, contrast \
that reads on dark mode). Never mention audio, BPM, tempo, music, songs, text or \
watermarks.",
        cfg.image_style
    );
    let user =
        format!("Thumbnail concept, keep the scene but adapt the palette and props to it: {vibe}");
    let body = json!({
        "model": cfg.llm_model,
        "temperature": 0.5,
        "max_tokens": 500,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ]
    });
    let resp: serde_json::Value = long_client()?
        .post(&url)
        .json(&body)
        .send()
        .and_then(|r| r.error_for_status())
        .with_context(|| format!("llama-server failed at {url}"))?
        .json()?;
    let raw = resp["choices"][0]["message"]["content"]
        .as_str()
        .context("LLM response without content")?;
    let text = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches('"')
        .to_string();
    if text.is_empty() {
        bail!("the LLM returned an empty prompt");
    }
    Ok(sanitize_eye_prompt(&text))
}

/// The 4B likes to write "glowing red eyes" for dramatic prompts, and the image
/// model renders it literally as pixel crosses. Keep the eyes natural without
/// touching the rest of the scene. Longest phrases go first so the generic
/// "eyes" rules do not leave a half-replaced phrase behind.
pub fn sanitize_eye_prompt(prompt: &str) -> String {
    prompt
        .replace("glowing red eyes", "soft green eyes")
        .replace("Glowing red eyes", "soft green eyes")
        .replace("glowing pixel eyes", "soft eyes")
        .replace("Glowing pixel eyes", "soft eyes")
        .replace("pixel eyes", "soft eyes")
        .replace("Pixel eyes", "soft eyes")
        .replace("glowing eyes", "soft eyes")
        .replace("Glowing eyes", "soft eyes")
        .replace("neon eyes", "soft eyes")
        .replace("empty eyes", "soft eyes")
        .replace("red eyes", "soft green eyes")
}

/// Asks for the YouTube copy in the same loaded LLM session as the thumbnail
/// prompt. The reply is a JSON object so the fixed template can stay in
/// `youtube.rs` instead of inside the prompt.
pub fn ask_youtube(cfg: &Config, vibe: &str) -> Result<crate::youtube::Draft> {
    let url = format!("{}/v1/chat/completions", cfg.llm_url.trim_end_matches('/'));
    let body = json!({
        "model": cfg.llm_model,
        "temperature": 0.8,
        "max_tokens": 700,
        "messages": [
            {"role": "system", "content": crate::youtube::SYSTEM},
            {"role": "user", "content": crate::youtube::user_prompt(vibe)}
        ]
    });
    let resp: serde_json::Value = long_client()?
        .post(&url)
        .json(&body)
        .send()
        .and_then(|r| r.error_for_status())
        .with_context(|| format!("llama-server failed at {url}"))?
        .json()?;
    let raw = resp["choices"][0]["message"]["content"]
        .as_str()
        .context("LLM response without content")?;
    parse_youtube(raw)
}

/// Models sometimes wrap the JSON in a markdown fence or add a sentence around
/// it. Take the outermost object and ignore the rest.
pub fn parse_youtube(raw: &str) -> Result<crate::youtube::Draft> {
    let start = raw.find('{').context("the LLM reply has no JSON object")?;
    let end = raw
        .rfind('}')
        .context("the LLM reply has no closing brace")?;
    if end <= start {
        bail!("the LLM reply has an empty JSON object");
    }
    let draft: crate::youtube::Draft =
        serde_json::from_str(&raw[start..=end]).context("the LLM reply is not valid JSON")?;
    if draft.title.trim().is_empty() && draft.intro.trim().is_empty() && draft.hashtags.is_empty() {
        bail!("the LLM returned an empty metadata object");
    }
    Ok(draft)
}

/// One take: drafts and the final share the same request shape, only the
/// duration, steps and sampler change.
pub struct AudioRequest<'a> {
    pub prompt: &'a str,
    pub reference: &'a str,
    pub seed: u32,
    pub duration_sec: u32,
    pub steps: u32,
    pub sampler: &'a str,
}

pub fn generate_audio(cfg: &Config, take: AudioRequest<'_>) -> Result<Vec<u8>> {
    let url = format!("{}/v1/tasks/run", cfg.audio_url.trim_end_matches('/'));
    // The server only forwards the nested `options` object to the ACE-Step
    // parser. duration_sec, reference_audio and sampler_mode are ignored when
    // they sit at the top level of `request`.
    let body = json!({
        "model": cfg.audio_model,
        "request": {
            "route": "text2music",
            "text": take.prompt,
            "seed": take.seed,
            "num_inference_steps": take.steps,
            "guidance_scale": cfg.guidance,
            "options": {
                "reference_audio": take.reference,
                "duration_sec": take.duration_sec,
                "sampler_mode": take.sampler,
                "negative_prompt": cfg.audio_negative_prompt
            }
        }
    });
    let resp: serde_json::Value = long_client()?
        .post(&url)
        .json(&body)
        .send()
        .and_then(|r| r.error_for_status())
        .with_context(|| format!("audio.cpp failed at {url}"))?
        .json()?;
    let b64 = resp["audio"]
        .as_str()
        .context("response without an audio field")?;
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .context("invalid base64 audio")
}

/// sd-server strips this block before generation. It carries the native
/// stable-diffusion.cpp fields that the OpenAI endpoint does not expose.
fn with_extra_args(prompt: &str, seed: u32, steps: u32, negative_prompt: &str) -> String {
    let mut extra = json!({
        "seed": seed,
        "sample_params": { "sample_steps": steps }
    });
    if !negative_prompt.trim().is_empty() {
        extra["negative_prompt"] = json!(negative_prompt);
    }
    format!("{prompt} <sd_cpp_extra_args>{extra}</sd_cpp_extra_args>")
}

/// One image with an explicit negative prompt, so a retry can add to the
/// configured one without mutating the config. A non-empty reference list uses
/// the sdapi endpoint, which is the one that carries `extra_images`.
pub fn generate_image(
    cfg: &Config,
    prompt: &str,
    negative_prompt: &str,
    seed: u32,
    references: &[PathBuf],
) -> Result<Vec<u8>> {
    if !references.is_empty() {
        return generate_image_with_references(cfg, prompt, negative_prompt, seed, references);
    }
    let url = format!("{}/v1/images/generations", cfg.sd_url.trim_end_matches('/'));
    let prompt = with_extra_args(prompt, seed, cfg.image_steps, negative_prompt);
    let body = json!({
        "prompt": prompt,
        "n": 1,
        "size": format!("{}x{}", cfg.width, cfg.height),
        "output_format": "png"
    });
    let resp: serde_json::Value = long_client()?
        .post(&url)
        .json(&body)
        .send()
        .and_then(|r| r.error_for_status())
        .with_context(|| format!("sd-server failed at {url}"))?
        .json()?;
    let b64 = resp["data"][0]["b64_json"]
        .as_str()
        .context("response without data[0].b64_json")?;
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .context("invalid base64 image")
}

fn generate_image_with_references(
    cfg: &Config,
    prompt: &str,
    negative_prompt: &str,
    seed: u32,
    references: &[PathBuf],
) -> Result<Vec<u8>> {
    let url = format!("{}/sdapi/v1/txt2img", cfg.sd_url.trim_end_matches('/'));
    let extra_images = references
        .iter()
        .map(|path| {
            let bytes = std::fs::read(path)
                .with_context(|| format!("reading reference {}", path.display()))?;
            Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
        })
        .collect::<Result<Vec<String>>>()?;
    let body = json!({
        "prompt": prompt,
        "negative_prompt": negative_prompt,
        "width": cfg.width,
        "height": cfg.height,
        "steps": cfg.image_steps,
        "seed": seed,
        "extra_images": extra_images,
    });
    let resp: serde_json::Value = long_client()?
        .post(&url)
        .json(&body)
        .send()
        .and_then(|r| r.error_for_status())
        .with_context(|| format!("sd-server (references) failed at {url}"))?
        .json()?;
    let b64 = resp["images"][0]
        .as_str()
        .context("sdapi response without images[0]")?;
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .context("invalid base64 image")
}

#[cfg(test)]
#[path = "tests/clients.rs"]
mod tests;
