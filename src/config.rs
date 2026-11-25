use std::path::PathBuf;

use serde::Deserialize;

#[derive(Clone, Debug)]
pub struct Config {
    /// audio.cpp server (ACE-Step) on the GPU VM.
    pub audio_url: String,
    pub audio_model: String,
    /// stable-diffusion.cpp server on the GPU VM.
    pub sd_url: String,
    /// llama-server on the GPU VM, used to write the image prompt.
    pub llm_url: String,
    pub llm_model: String,
    /// SSH target used to stage the reference audio on the VM.
    pub ssh_host: String,
    pub remote_ref_dir: String,
    pub out_dir: PathBuf,
    /// SQLite file behind `melodias history`.
    pub db_path: PathBuf,
    /// Optional lifecycle commands run over SSH between GPU stages.
    pub llm_start_cmd: String,
    pub llm_stop_cmd: String,
    pub sd_start_cmd: String,
    pub sd_stop_cmd: String,
    pub audio_start_cmd: String,
    pub audio_stop_cmd: String,
    /// Fixed medium for the music prompt, independent of the vibe.
    pub audio_style: String,
    /// Run-specific extras appended verbatim to the music prompt (BPM, instruments).
    pub audio_detail: String,
    /// Fixed style for the thumbnail prompt, pinned in the system message.
    pub image_style: String,
    /// Model labels for the YouTube copy; they are copy, not request parameters.
    pub image_model: String,
    pub creative_model: String,
    /// Negative prompt for sd-server, sent through sd_cpp_extra_args.
    pub image_negative_prompt: String,
    /// Steers ACE-Step away from voices; the channel only publishes instrumentals.
    pub audio_negative_prompt: String,
    /// Label shown in the header and the splash, for example "Tesla P40".
    pub gpu_label: String,
    pub duration_sec: u32,
    pub steps: u32,
    pub guidance: f32,
    pub sampler: String,
    pub width: u32,
    pub height: u32,
    pub video_width: u32,
    pub video_height: u32,
    /// `remotion` (default) or `ffmpeg`; ffmpeg is explicit only.
    pub video_renderer: String,
    /// `both` (default), `bokeh` or `sparks`.
    pub video_particles: String,
    /// Remotion project, relative to the working directory.
    pub video_project: PathBuf,
    pub judge_enabled: bool,
    /// `auto`, `opencode` or `openrouter`.
    pub judge_provider: String,
    pub opencode_api_key: String,
    pub openrouter_api_key: String,
    /// Optional key for `melodias refs fetch --source pixabay`.
    pub pixabay_api_key: String,
    /// Model id for the OpenCode Go backend (Gemini native API).
    pub judge_model: String,
    pub judge_omni_model: String,
    pub judge_reasoner_model: String,
    pub judge_image_px: u32,
    pub judge_audio_sec: u32,
    pub judge_timeout_sec: u64,
    pub image_steps: u32,
    /// Optional fixed seed to reproduce an image candidate.
    pub image_seed: Option<u32>,
    /// Optional folder of reference thumbnails, grouped by concept.
    pub image_references: PathBuf,
    /// How many references to send per generation.
    pub image_reference_count: u32,
    /// Force a concept subfolder instead of matching the vibe.
    pub image_reference_concept: String,
    /// Number of candidates per modality.
    pub candidates: u32,
    /// Deterministic retry rounds; 0 disables the loop.
    pub score_rounds: u32,
    pub audio_draft_sec: u32,
    pub audio_draft_steps: u32,
    pub audio_draft_sampler: String,
    pub reference_sec: u32,
    /// Optional WAV used instead of a window of the source video.
    pub audio_reference: PathBuf,
    pub fade_in_sec: f64,
    pub fade_out_sec: f64,
    pub loudness_lufs: f64,
}

impl Config {
    pub fn from_env() -> Self {
        let file = FileConfig::load();
        Self {
            audio_url: pick(
                env("AUDIO_URL"),
                file.audio_url,
                "http://192.168.88.38:8090",
            ),
            audio_model: pick(env("AUDIO_MODEL"), file.audio_model, "ace-step"),
            sd_url: pick(env("SD_URL"), file.sd_url, "http://192.168.88.38:1234"),
            llm_url: pick(env("LLM_URL"), file.llm_url, "http://192.168.88.38:8080"),
            llm_model: pick(env("LLM_MODEL"), file.llm_model, "local"),
            ssh_host: pick(env("SSH_HOST"), file.ssh_host, "root@192.168.88.38"),
            remote_ref_dir: pick(env("REMOTE_REF_DIR"), file.remote_ref_dir, "/models/ref"),
            out_dir: env("OUT_DIR")
                .map(PathBuf::from)
                .or(file.out_dir)
                .unwrap_or_else(|| PathBuf::from("output")),
            db_path: env("MELODIAS_DB")
                .map(PathBuf::from)
                .or(file.db_path)
                .unwrap_or_else(Self::default_db_path),
            llm_start_cmd: pick(
                env("LLM_START_CMD"),
                file.llm_start_cmd,
                "/home/ubuntu/bin/serve-llm.sh start",
            ),
            llm_stop_cmd: pick(
                env("LLM_STOP_CMD"),
                file.llm_stop_cmd,
                "/home/ubuntu/bin/serve-llm.sh stop",
            ),
            sd_start_cmd: pick(
                env("SD_START_CMD"),
                file.sd_start_cmd,
                "/home/ubuntu/bin/serve-sd.sh start",
            ),
            sd_stop_cmd: pick(
                env("SD_STOP_CMD"),
                file.sd_stop_cmd,
                "/home/ubuntu/bin/serve-sd.sh stop",
            ),
            audio_start_cmd: pick(
                env("AUDIO_START_CMD"),
                file.audio_start_cmd,
                "/home/ubuntu/bin/serve-audio.sh start",
            ),
            audio_stop_cmd: pick(
                env("AUDIO_STOP_CMD"),
                file.audio_stop_cmd,
                "/home/ubuntu/bin/serve-audio.sh stop",
            ),
            audio_style: pick(env("AUDIO_STYLE"), file.audio_style, "8-bit chiptune"),
            audio_detail: pick(env("AUDIO_DETAIL"), file.audio_detail, ""),
            image_style: pick(env("IMAGE_STYLE"), file.image_style, "8-bit pixel art"),
            image_model: pick(
                env("IMAGE_MODEL"),
                file.image_model,
                "MOSS-Image-SFT-UD-Q4_K_XL",
            ),
            creative_model: pick(
                env("CREATIVE_MODEL"),
                file.creative_model,
                "Qwen3-4B-Instruct-2507",
            ),
            image_negative_prompt: pick(
                env("IMAGE_NEGATIVE_PROMPT"),
                file.image_negative_prompt,
                "glowing eyes, neon eyes, empty eyes, oversaturated eyes, text, watermark, signature, extra fingers, deformed hands",
            ),
            audio_negative_prompt: pick(
                env("AUDIO_NEGATIVE_PROMPT"),
                file.audio_negative_prompt,
                "vocals, singing, rap, spoken words, choir, lyrics, humming, vocal chops, voice, \
                 acoustic folk, ambient music, lo-fi production, trap beats, hip hop drums, EDM \
                 drops, dubstep, generic pop structure, repetitive four-chord pop progression, \
                 weak percussion, muddy bass, distorted muddy guitars, excessive reverb, harsh \
                 clipping, synthetic cheap-sounding instruments, monotonous melodies, overly slow \
                 tempo, excessive silence, unfinished arrangement, abrupt ending, poor mixing, low \
                 fidelity, amateur production",
            ),
            gpu_label: pick(env("GPU_LABEL"), file.gpu_label, "local models"),
            duration_sec: pick_parsed(env("DURATION_SEC"), file.duration_sec, 210),
            steps: pick_parsed(env("STEPS"), file.steps, 32),
            guidance: pick_parsed(env("GUIDANCE"), file.guidance, 5.0),
            sampler: pick(env("SAMPLER"), file.sampler, "heun"),
            width: pick_parsed(env("WIDTH"), file.width, 1280),
            height: pick_parsed(env("HEIGHT"), file.height, 720),
            video_width: pick_parsed(env("VIDEO_WIDTH"), file.video_width, 1920),
            video_height: pick_parsed(env("VIDEO_HEIGHT"), file.video_height, 1080),
            video_renderer: pick(env("VIDEO_RENDERER"), file.video_renderer, "remotion"),
            video_particles: pick(env("VIDEO_PARTICLES"), file.video_particles, "both"),
            video_project: env("VIDEO_PROJECT")
                .map(PathBuf::from)
                .or(file.video_project)
                .unwrap_or_else(|| PathBuf::from("video")),
            // On by default: the judge itself is skipped when no key is set.
            judge_enabled: env("JUDGE_ENABLED")
                .map(|value| parse_bool(&value))
                .or(file.judge_enabled)
                .unwrap_or(true),
            judge_provider: pick(env("JUDGE_PROVIDER"), file.judge_provider, "auto"),
            opencode_api_key: env("OPENCODE_API_KEY")
                .or(file.opencode_api_key)
                .unwrap_or_default(),
            openrouter_api_key: env("OPENROUTER_API_KEY")
                .or(file.openrouter_api_key)
                .unwrap_or_default(),
            pixabay_api_key: env("PIXABAY_API_KEY")
                .or(file.pixabay_api_key)
                .unwrap_or_default(),
            judge_model: pick(env("JUDGE_MODEL"), file.judge_model, "gemini-4-argon"),
            judge_omni_model: pick(
                env("JUDGE_OMNI_MODEL"),
                file.judge_omni_model,
                "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free",
            ),
            judge_reasoner_model: pick(
                env("JUDGE_REASONER_MODEL"),
                file.judge_reasoner_model,
                "nvidia/nemotron-3-ultra-550b-a55b:free",
            ),
            judge_image_px: pick_parsed(env("JUDGE_IMAGE_PX"), file.judge_image_px, 768),
            judge_audio_sec: pick_parsed(env("JUDGE_AUDIO_SEC"), file.judge_audio_sec, 15),
            judge_timeout_sec: pick_parsed(env("JUDGE_TIMEOUT_SEC"), file.judge_timeout_sec, 300),
            image_steps: pick_parsed(env("IMAGE_STEPS"), file.image_steps, 20),
            image_seed: env("IMAGE_SEED")
                .and_then(|value| value.parse().ok())
                .or(file.image_seed),
            image_references: env("IMAGE_REFERENCES")
                .map(PathBuf::from)
                .or(file.image_references)
                .unwrap_or_default(),
            image_reference_count: pick_parsed(
                env("IMAGE_REFERENCE_COUNT"),
                file.image_reference_count,
                3,
            ),
            image_reference_concept: pick(
                env("IMAGE_REFERENCE_CONCEPT"),
                file.image_reference_concept,
                "",
            ),
            candidates: pick_parsed(env("CANDIDATES"), file.candidates, 3),
            score_rounds: pick_parsed(env("SCORE_ROUNDS"), file.score_rounds, 2),
            audio_draft_sec: pick_parsed(env("AUDIO_DRAFT_SEC"), file.audio_draft_sec, 60),
            audio_draft_steps: pick_parsed(env("AUDIO_DRAFT_STEPS"), file.audio_draft_steps, 16),
            audio_draft_sampler: pick(
                env("AUDIO_DRAFT_SAMPLER"),
                file.audio_draft_sampler,
                "euler",
            ),
            reference_sec: pick_parsed(env("REFERENCE_SEC"), file.reference_sec, 60),
            audio_reference: env("AUDIO_REFERENCE")
                .map(PathBuf::from)
                .or(file.audio_reference)
                .unwrap_or_default(),
            fade_in_sec: pick_parsed(env("FADE_IN_SEC"), file.fade_in_sec, 1.5),
            fade_out_sec: pick_parsed(env("FADE_OUT_SEC"), file.fade_out_sec, 3.0),
            loudness_lufs: pick_parsed(env("LOUDNESS_LUFS"), file.loudness_lufs, -15.0),
        }
    }

    pub fn config_path() -> PathBuf {
        if let Some(path) = env("MELODIAS_CONFIG") {
            return PathBuf::from(path);
        }
        let base = env("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env("HOME").map(|home| PathBuf::from(home).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("melodias-studio").join("config.toml")
    }

    /// Next to `config.toml`, or in the directory of `MELODIAS_CONFIG`.
    pub fn default_db_path() -> PathBuf {
        Self::config_path().with_file_name("history.db")
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

fn pick(env_value: Option<String>, file_value: Option<String>, default: &str) -> String {
    env_value
        .or(file_value)
        .unwrap_or_else(|| default.to_string())
}

fn pick_parsed<T>(env_value: Option<String>, file_value: Option<T>, default: T) -> T
where
    T: std::str::FromStr,
{
    env_value
        .and_then(|value| value.parse().ok())
        .or(file_value)
        .unwrap_or(default)
}

/// Everything in `~/.config/melodias-studio/config.toml` is optional. Environment
/// variables win, then the file, then the defaults above.
#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    audio_url: Option<String>,
    audio_model: Option<String>,
    sd_url: Option<String>,
    llm_url: Option<String>,
    llm_model: Option<String>,
    ssh_host: Option<String>,
    remote_ref_dir: Option<String>,
    out_dir: Option<PathBuf>,
    db_path: Option<PathBuf>,
    llm_start_cmd: Option<String>,
    llm_stop_cmd: Option<String>,
    sd_start_cmd: Option<String>,
    sd_stop_cmd: Option<String>,
    audio_start_cmd: Option<String>,
    audio_stop_cmd: Option<String>,
    audio_style: Option<String>,
    audio_detail: Option<String>,
    image_style: Option<String>,
    image_model: Option<String>,
    creative_model: Option<String>,
    image_negative_prompt: Option<String>,
    audio_negative_prompt: Option<String>,
    gpu_label: Option<String>,
    duration_sec: Option<u32>,
    steps: Option<u32>,
    guidance: Option<f32>,
    sampler: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    video_width: Option<u32>,
    video_height: Option<u32>,
    video_renderer: Option<String>,
    video_particles: Option<String>,
    video_project: Option<PathBuf>,
    judge_enabled: Option<bool>,
    judge_provider: Option<String>,
    opencode_api_key: Option<String>,
    openrouter_api_key: Option<String>,
    pixabay_api_key: Option<String>,
    judge_model: Option<String>,
    judge_omni_model: Option<String>,
    judge_reasoner_model: Option<String>,
    judge_image_px: Option<u32>,
    judge_audio_sec: Option<u32>,
    judge_timeout_sec: Option<u64>,
    image_steps: Option<u32>,
    image_seed: Option<u32>,
    image_references: Option<PathBuf>,
    image_reference_count: Option<u32>,
    image_reference_concept: Option<String>,
    candidates: Option<u32>,
    score_rounds: Option<u32>,
    audio_draft_sec: Option<u32>,
    audio_draft_steps: Option<u32>,
    audio_draft_sampler: Option<String>,
    reference_sec: Option<u32>,
    audio_reference: Option<PathBuf>,
    fade_in_sec: Option<f64>,
    fade_out_sec: Option<f64>,
    loudness_lufs: Option<f64>,
}

impl FileConfig {
    fn load() -> Self {
        let path = Config::config_path();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(err) => {
                eprintln!("warning: could not read {}: {err}", path.display());
                return Self::default();
            }
        };
        match toml::from_str(&text) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("warning: invalid config at {}: {err}", path.display());
                Self::default()
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;
