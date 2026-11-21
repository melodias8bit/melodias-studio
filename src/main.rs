mod app;
mod backfill;
mod banner;
mod chip;
mod clients;
mod config;
mod dashboard;
mod history;
mod judge;
mod media;
mod pipeline;
mod references;
mod refs;
mod retry;
mod score;
mod video;

use std::fs::{File, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

use crossterm::style::Stylize;

use pipeline::{Msg, State};

const USAGE: &str = "usage: melodias                 (TUI)\n       melodias <url> <vibe>     (headless)\n       melodias image <dir>      (regenerate the thumbnail of a run)\n       melodias judge <dir>      (review a finished run with the judge)\n       melodias score <dir>      (score a finished run, no GPU)\n       melodias history [view]   (runs, findings or trend, from the local database)
       melodias backfill [dir]   (score finished runs into the history database)\n       melodias dashboard        (local read-only charts over the database)\n       melodias render <dir>     (produced video with Remotion)\n       melodias video <dir>      (fast video with ffmpeg)\n       melodias refs fetch       (fill the reference library from a source)\n       melodias refs import      (import files or packs into the library)\n       melodias refs generate    (generate reference images with sd-server)
       melodias chip <url|file>  (experimental NES cover of a track)";

fn main() -> io::Result<()> {
    let cfg = config::Config::from_env();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args
        .first()
        .is_some_and(|arg| arg == "-h" || arg == "--help")
    {
        println!("{USAGE}");
        return Ok(());
    }
    if args.first().is_some_and(|arg| arg == "image") {
        return run_image(cfg, &args);
    }
    if args.first().is_some_and(|arg| arg == "judge") {
        return run_judge(cfg, &args);
    }
    if args.first().is_some_and(|arg| arg == "score") {
        return run_score(cfg, &args);
    }
    if args.first().is_some_and(|arg| arg == "history") {
        return run_history(cfg, &args);
    }
    if args.first().is_some_and(|arg| arg == "backfill") {
        return run_backfill(cfg, &args);
    }
    if args.first().is_some_and(|arg| arg == "dashboard") {
        return run_dashboard(cfg, &args);
    }
    if args.first().is_some_and(|arg| arg == "render") {
        return run_render(cfg, &args);
    }
    if args.first().is_some_and(|arg| arg == "video") {
        return run_video(cfg, &args);
    }
    if args.first().is_some_and(|arg| arg == "refs") {
        return run_refs(cfg, &args);
    }
    if args.first().is_some_and(|arg| arg == "chip") {
        return run_chip(cfg, &args);
    }
    if !args.is_empty() {
        return run_headless(cfg, &args);
    }

    let mut terminal = ratatui::init();
    let result = app::run(&mut terminal, cfg);
    ratatui::restore();
    result
}

/// Runs the same pipeline the TUI drives, with a compact terminal view. Stages
/// are printed as groups with icons and color, and the plain text goes to the
/// file named by MELODIAS_LOG when it is set.
fn run_headless(cfg: config::Config, args: &[String]) -> io::Result<()> {
    let args: Vec<&String> = args.iter().filter(|arg| *arg != "--headless").collect();
    let [url, vibe] = args.as_slice() else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };

    let host = cfg.ssh_host.clone();
    let gpu = cfg.gpu_label.clone();
    let log_path = std::env::var_os("MELODIAS_LOG").map(PathBuf::from);
    if let Some(path) = &log_path {
        println!("log: {}", path.display());
    }
    let mut out = Reporter::new(log_path)?;

    banner::print(out.pretty, &gpu);

    let title = format!("♪ melodias-studio · {host}");
    let colored = out.pretty.then(|| title.as_str().cyan().bold().to_string());
    out.line(&title, colored);
    let separator = "────────────────────────────────────────";
    let colored = out.pretty.then(|| separator.dark_grey().to_string());
    out.line(separator, colored);

    let (tx, rx) = channel();
    let decisions = pipeline::spawn(cfg, url.trim().to_string(), vibe.trim().to_string(), tx);

    for msg in rx {
        match msg {
            Msg::Stage(idx, state) => out.stage(idx, state),
            Msg::Log(text) => out.log_line(&text),
            Msg::Choose(request) => {
                out.line("", None);
                let pool = &request.pool;
                let header = format!(
                    "Review: {} images and {} audio drafts",
                    pool.images.len(),
                    pool.audios.len()
                );
                let colored = out
                    .pretty
                    .then(|| header.as_str().cyan().bold().to_string());
                out.line(&header, colored);
                for (index, candidate) in pool.images.iter().enumerate() {
                    out.log_line(&format!(
                        "image {}: {} (seed {})",
                        index + 1,
                        candidate.file.display(),
                        candidate.seed
                    ));
                }
                for (index, candidate) in pool.audios.iter().enumerate() {
                    out.log_line(&format!(
                        "audio {}: {} (seed {})",
                        index + 1,
                        candidate.file.display(),
                        candidate.seed
                    ));
                }
                if let Some(note) = &request.note {
                    out.log_line(&format!("review: {note}"));
                }
                let selection = request.suggestion.unwrap_or(request.default);
                let _ = decisions.send(selection);
                out.log_line(&format!(
                    "selected image {} and audio draft {} (headless)",
                    selection.image + 1,
                    selection.audio + 1
                ));
            }
            Msg::Done(Ok(outcome)) => {
                out.line("", None);
                let line = format!(
                    "✔ done · seed {} · {}",
                    outcome.seed,
                    outcome.audio.display()
                );
                let colored = out.pretty.then(|| line.as_str().green().bold().to_string());
                out.line(&line, colored);
                let image = format!("✔ thumbnail · {}", outcome.image.display());
                let colored = out.pretty.then(|| image.as_str().green().to_string());
                out.line(&image, colored);
                let video = format!("✔ video · {}", outcome.video.display());
                let colored = out.pretty.then(|| video.as_str().green().to_string());
                out.line(&video, colored);
                return Ok(());
            }
            Msg::Done(Err(err)) => {
                out.error(&format!("✖ failed: {err}"));
                std::process::exit(1);
            }
        }
    }
    Ok(())
}

/// Serves the read-only dashboard on localhost. It never writes the database.
fn run_dashboard(cfg: config::Config, args: &[String]) -> io::Result<()> {
    let mut port = dashboard::DEFAULT_PORT;
    let mut limit = dashboard::DEFAULT_LIMIT;
    let mut index = 1;
    while let Some(arg) = args.get(index) {
        match arg.as_str() {
            "--port" => port = next_value(args, &mut index, "--port"),
            "--limit" => limit = next_value(args, &mut index, "--limit"),
            other => {
                eprintln!("usage: melodias dashboard [--port {port}] [--limit {limit}]");
                eprintln!("unknown option: {other}");
                std::process::exit(2);
            }
        }
        index += 1;
    }
    if port == 0 {
        eprintln!("--port must be between 1 and 65535");
        std::process::exit(2);
    }
    dashboard::serve(cfg.db_path.clone(), cfg.out_dir.clone(), port, limit.max(1))
        .map_err(|error| io::Error::other(format!("{error:#}")))
}

fn next_value<T: std::str::FromStr>(args: &[String], index: &mut usize, flag: &str) -> T {
    *index += 1;
    let Some(value) = args.get(*index).and_then(|value| value.parse::<T>().ok()) else {
        eprintln!("{flag} needs a valid value");
        std::process::exit(2);
    };
    value
}

/// Scores a finished run with the deterministic rubric: every candidate and
/// the final pair. No GPU, no cloud, and the only write is work/score.json.
fn run_score(cfg: config::Config, args: &[String]) -> io::Result<()> {
    let as_json = args.iter().any(|arg| arg == "--json");
    let Some(target) = args.get(1).filter(|arg| !arg.starts_with("--")) else {
        eprintln!("usage: melodias score <output-dir> [--json]");
        std::process::exit(2);
    };
    let requested = PathBuf::from(target);
    let dir = if requested.is_dir() {
        requested
    } else {
        cfg.out_dir.join(target)
    };
    let dir = std::fs::canonicalize(&dir)?;
    let run = score::score_run(&dir).map_err(|error| io::Error::other(format!("{error:#}")))?;
    let path =
        score::write_json(&dir, &run).map_err(|error| io::Error::other(format!("{error:#}")))?;
    if as_json {
        println!("{}", serde_json::to_string_pretty(&run)?);
        return Ok(());
    }
    print_score(&run);
    println!("wrote {}", path.display());
    Ok(())
}

/// Reads the local history database. Never writes to it.
fn run_history(cfg: config::Config, args: &[String]) -> io::Result<()> {
    let mut mode = "runs";
    let mut format = history::Format::Table;
    let mut by = history::By::Day;
    let mut index = 1;
    while let Some(arg) = args.get(index) {
        match arg.as_str() {
            "--json" => format = history::Format::Json,
            "--csv" => format = history::Format::Csv,
            "--by" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("usage: melodias history trend --by day|week|month");
                    std::process::exit(2);
                };
                by = history::By::parse(value).unwrap_or_else(|| {
                    eprintln!("unknown period: {value}, expected day, week or month");
                    std::process::exit(2);
                });
            }
            other if other.starts_with('-') => {
                eprintln!("unknown option: {other}");
                std::process::exit(2);
            }
            other => mode = other,
        }
        index += 1;
    }
    let db = &cfg.db_path;
    if !db.is_file() && format == history::Format::Table {
        println!("no history at {}", db.display());
        println!("the next finished run writes it, or use make backfill");
        return Ok(());
    }
    let text = (|| -> anyhow::Result<String> {
        match mode {
            "runs" => history::render_runs(&history::recent(db, 20)?, format),
            "findings" => history::render_findings(&history::findings(db)?, format),
            "trend" => history::render_trend(&history::trend(db, by)?, format),
            other => {
                eprintln!("unknown history view: {other}, expected runs, findings or trend");
                std::process::exit(2);
            }
        }
    })()
    .map_err(|error| io::Error::other(format!("{error:#}")))?;
    println!("{}", text.trim_end_matches('\n'));
    Ok(())
}

fn print_score(run: &score::RunScore) {
    println!("score: {}", run.dir);
    for candidate in &run.images {
        println!(
            "  image {:>2} {:>5.1}  {}{}",
            candidate.index,
            candidate.score,
            file_name(&candidate.file),
            if candidate.selected {
                "  (selected)"
            } else {
                ""
            }
        );
    }
    for candidate in &run.audios {
        println!(
            "  audio {:>2} {:>5.1}  {}",
            candidate.index,
            candidate.score,
            file_name(&candidate.file)
        );
    }
    println!(
        "final: image {:.1} and audio {:.1}",
        run.final_image.score, run.final_audio.score
    );
    let editorial = run
        .editorial
        .map(|value| format!("{value:.1}"))
        .unwrap_or_else(|| "-".to_string());
    println!(
        "technical {:.1} · editorial {} · total {:.1} · {}",
        run.technical,
        editorial,
        run.total,
        if run.accepted {
            "accepted"
        } else {
            "below target"
        }
    );
    if !run.gates.is_empty() {
        println!("gates");
        for gate in &run.gates {
            println!("  {}  {}", gate.code, gate.message);
        }
    }
    let mut findings: Vec<&score::Finding> = run
        .final_image
        .findings
        .iter()
        .chain(run.final_audio.findings.iter())
        .collect();
    findings.sort_by(|a, b| a.delta.total_cmp(&b.delta));
    if !findings.is_empty() {
        println!("findings");
        for finding in findings.iter().take(6) {
            println!(
                "  {:<14} {:>6.1}  {}",
                finding.code, finding.delta, finding.message
            );
        }
    }
    let skipped: Vec<&str> = run
        .final_image
        .skipped
        .iter()
        .chain(run.final_audio.skipped.iter())
        .copied()
        .collect();
    if !skipped.is_empty() {
        println!("not evaluated: {}", skipped.join(", "));
    }
}

fn file_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

/// Runs the OpenRouter judge on the candidates of a finished run.
fn run_judge(cfg: config::Config, args: &[String]) -> io::Result<()> {
    let [_, target] = args else {
        eprintln!("usage: melodias judge <output-dir>");
        std::process::exit(2);
    };
    if !judge::enabled(&cfg) {
        eprintln!(
            "the judge has no API key: set OPENCODE_API_KEY (preferred) or OPENROUTER_API_KEY"
        );
        std::process::exit(2);
    }
    let requested = PathBuf::from(target);
    let dir = if requested.is_dir() {
        requested
    } else {
        cfg.out_dir.join(target)
    };
    let dir = std::fs::canonicalize(&dir)?;
    let meta: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("meta.json"))?)?;
    let vibe = meta["vibe"]
        .as_str()
        .unwrap_or("original 8-bit instrumental");
    let candidates = |value: &serde_json::Value| -> Vec<pipeline::Candidate> {
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|candidate| {
                let file = PathBuf::from(candidate["file"].as_str()?);
                let file = dir
                    .join("candidates")
                    .join(file.file_name().unwrap_or_default());
                Some(pipeline::Candidate {
                    file,
                    seed: candidate["seed"].as_u64()? as u32,
                })
            })
            .collect()
    };
    let images = candidates(&meta["image"]["candidates"]);
    let audios = candidates(&meta["audio"]["draft"]["candidates"]);
    if images.is_empty() || audios.is_empty() {
        eprintln!("meta.json has no image or audio candidates");
        std::process::exit(2);
    }
    let work = dir.join("work");
    let verdict = judge::decide(
        &cfg,
        judge::Request {
            vibe,
            images: &images,
            audios: &audios,
            work: &work,
            findings: None,
        },
    )
    .map_err(|error| io::Error::other(format!("{error:#}")))?;
    println!(
        "judge: image {} · audio {} · score {}{} · {}",
        verdict.image + 1,
        verdict.audio + 1,
        verdict.score,
        verdict.must_fix_suffix(),
        verdict.reason
    );
    println!("raw: {}", dir.join("work/judge/verdict.json").display());
    Ok(())
}

/// Renders the produced video with the Remotion project: overlay clips, an
/// audio-reactive waveform and a slow zoom. Slower than ffmpeg.
fn run_render(cfg: config::Config, args: &[String]) -> io::Result<()> {
    let [_, target] = args else {
        eprintln!("usage: melodias render <output-dir>");
        std::process::exit(2);
    };
    let requested = PathBuf::from(target);
    let dir = if requested.is_dir() {
        requested
    } else {
        cfg.out_dir.join(target)
    };
    let dir = std::fs::canonicalize(&dir)?;
    let image = dir.join("thumbnail.png");
    let audio = dir.join("audio.mp3");
    for path in [&image, &audio] {
        if !path.is_file() {
            eprintln!("missing {}", path.display());
            std::process::exit(2);
        }
    }
    std::fs::create_dir_all(dir.join("work"))?;
    let temp = dir.join("work").join("video-remotion.mp4");
    video::render_remotion(&cfg, &image, &audio, &temp, &mut |line| println!("{line}"))
        .map_err(|error| io::Error::other(format!("{error:#}")))?;
    let output = dir.join("video.mp4");
    if output.is_file() {
        let _ = std::fs::copy(&output, dir.join("video-ffmpeg.mp4"));
    }
    std::fs::rename(&temp, &output)?;
    let particles = video::particles_value(&cfg.video_particles)
        .map_err(|error| io::Error::other(format!("{error:#}")))?;
    let meta_path = dir.join("meta.json");
    if let Ok(text) = std::fs::read_to_string(&meta_path)
        && let Ok(mut meta) = serde_json::from_str::<serde_json::Value>(&text)
    {
        meta["video"]["renderer"] = serde_json::json!("remotion");
        meta["video"]["particles"] = serde_json::json!(particles);
        meta["video"]["file"] = serde_json::json!(output.display().to_string());
        std::fs::write(&meta_path, serde_json::to_vec_pretty(&meta)?)?;
    }
    println!("video: {}", output.display());
    Ok(())
}

/// Regenerates the thumbnail of a finished run from its stored prompt, keeping
/// the chosen seed, then rebuilds the video. It only uses sd-server.
fn run_image(cfg: config::Config, args: &[String]) -> io::Result<()> {
    let [_, target] = args else {
        eprintln!("usage: melodias image <output-dir>");
        std::process::exit(2);
    };
    let requested = PathBuf::from(target);
    let dir = if requested.is_dir() {
        requested
    } else {
        cfg.out_dir.join(target)
    };
    let meta_path = dir.join("meta.json");
    let text = std::fs::read_to_string(&meta_path)?;
    let mut meta: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| io::Error::other(format!("invalid {}: {error}", meta_path.display())))?;
    let selected = meta["image"]["selected"].as_u64().unwrap_or(1) as usize;
    let entry = meta["image"]["candidates"]
        .as_array()
        .and_then(|candidates| candidates.get(selected.saturating_sub(1)));
    let prompt = entry
        .and_then(|candidate| candidate["prompt"].as_str())
        .or_else(|| meta["image_prompt"].as_str())
        .ok_or_else(|| io::Error::other("meta.json has no image prompt"))?
        .to_string();
    let prompt = clients::sanitize_eye_prompt(&prompt);
    let negative = entry
        .and_then(|candidate| candidate["negative_prompt"].as_str())
        .or_else(|| meta["image"]["negative_prompt"].as_str())
        .unwrap_or(&cfg.image_negative_prompt)
        .to_string();
    let count = meta["image"]["candidates"]
        .as_array()
        .map(|candidates| candidates.len() + 1)
        .unwrap_or(1);
    let seed = entry
        .and_then(|candidate| candidate["seed"].as_u64())
        .unwrap_or(1) as u32;
    let references: Vec<PathBuf> = entry
        .and_then(|candidate| candidate["references"].as_array())
        .map(|list| {
            list.iter()
                .filter_map(|value| value.as_str())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default();
    let candidate = dir.join("candidates").join(format!("image-{count:02}.png"));

    media::ssh_run(&cfg, &cfg.audio_stop_cmd)
        .map_err(|error| io::Error::other(format!("{error:#}")))?;
    media::ssh_run(&cfg, &cfg.sd_start_cmd)
        .map_err(|error| io::Error::other(format!("{error:#}")))?;
    let generated = (|| -> anyhow::Result<()> {
        clients::wait_sd(&cfg, Duration::from_secs(180))?;
        let bytes = clients::generate_image(&cfg, &prompt, &negative, seed, &references)?;
        if let Some(parent) = candidate.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&candidate, &bytes)?;
        media::png_size(&candidate)?;
        Ok(())
    })();
    let stopped = media::ssh_run(&cfg, &cfg.sd_stop_cmd);
    generated.map_err(|error| io::Error::other(format!("{error:#}")))?;
    if let Err(error) = stopped {
        eprintln!("warning: sd-server: {error:#}");
    }

    let thumbnail = dir.join("thumbnail.png");
    let backup = dir.join("thumbnail-old.png");
    if thumbnail.is_file() {
        std::fs::copy(&thumbnail, &backup)?;
    }
    std::fs::copy(&candidate, &thumbnail)?;

    if let Some(candidates) = meta["image"]["candidates"].as_array_mut() {
        candidates.push(serde_json::json!({
            "file": candidate.display().to_string(),
            "seed": seed,
            "prompt": prompt,
            "negative_prompt": negative,
            "references": references
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>(),
        }));
    }
    meta["image"]["selected"] = serde_json::json!(count);
    meta["image"]["file"] = serde_json::json!(thumbnail.display().to_string());
    meta["image_prompt"] = serde_json::json!(prompt);
    std::fs::write(&meta_path, serde_json::to_vec_pretty(&meta)?)?;

    println!("image: {}", thumbnail.display());
    let audio = dir.join("audio.mp3");
    let rendered = video::render(&cfg, &thumbnail, &audio, &dir, &mut |line| {
        println!("{line}")
    })
    .map_err(|error| io::Error::other(format!("{error:#}")))?;
    meta["video"]["file"] = serde_json::json!(rendered.file.display().to_string());
    meta["video"]["renderer"] = serde_json::json!(rendered.renderer);
    meta["video"]["particles"] = serde_json::json!(rendered.particles);
    std::fs::write(&meta_path, serde_json::to_vec_pretty(&meta)?)?;
    println!("video: {}", rendered.file.display());
    Ok(())
}

/// Assembles the video of a finished run without touching the GPU.
fn run_video(cfg: config::Config, args: &[String]) -> io::Result<()> {
    let [_, target] = args else {
        eprintln!("usage: melodias video <output-dir>");
        std::process::exit(2);
    };
    let requested = std::path::PathBuf::from(target);
    let dir = if requested.is_dir() {
        requested
    } else {
        cfg.out_dir.join(target)
    };
    let image = dir.join("thumbnail.png");
    let audio = dir.join("audio.mp3");
    for path in [&image, &audio] {
        if !path.is_file() {
            eprintln!("missing {}", path.display());
            std::process::exit(2);
        }
    }
    let video = media::assemble_video(&cfg, &image, &audio, &dir)
        .map_err(|error| io::Error::other(format!("{error:#}")))?;
    println!("video: {}", video.display());
    Ok(())
}

struct Reporter {
    log: Option<File>,
    pretty: bool,
}

impl Reporter {
    fn new(path: Option<PathBuf>) -> io::Result<Self> {
        let log = match path {
            Some(path) => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                media::set_child_log(path.clone());
                Some(OpenOptions::new().create(true).append(true).open(path)?)
            }
            None => None,
        };
        Ok(Self {
            log,
            pretty: io::stdout().is_terminal(),
        })
    }

    fn line(&mut self, plain: &str, colored: Option<String>) {
        if let Some(file) = self.log.as_mut() {
            let _ = writeln!(file, "{plain}");
        }
        match colored {
            Some(text) => println!("{text}"),
            None => println!("{plain}"),
        }
    }

    fn error(&mut self, plain: &str) {
        if let Some(file) = self.log.as_mut() {
            let _ = writeln!(file, "{plain}");
        }
        if self.pretty {
            eprintln!("{}", plain.red().bold());
        } else {
            eprintln!("{plain}");
        }
    }

    fn stage(&mut self, idx: usize, state: State) {
        let Some(plain) = pipeline::stage_label(idx, state) else {
            return;
        };
        if state == State::Running {
            self.line("", None);
        }
        let colored = match state {
            State::Running => self
                .pretty
                .then(|| plain.as_str().yellow().bold().to_string()),
            State::Ok => self.pretty.then(|| plain.as_str().green().to_string()),
            State::Failed => self.pretty.then(|| plain.as_str().red().bold().to_string()),
            State::Pending => None,
        };
        self.line(&plain, colored);
    }

    fn log_line(&mut self, text: &str) {
        let plain = format!("   · {text}");
        let colored = self.pretty.then(|| plain.as_str().dark_grey().to_string());
        self.line(&plain, colored);
    }
}

fn run_refs(cfg: config::Config, args: &[String]) -> io::Result<()> {
    refs::cli::run(&cfg, &args[1..]).map_err(|error| io::Error::other(format!("{error:#}")))
}

fn run_chip(cfg: config::Config, args: &[String]) -> io::Result<()> {
    chip::run_chip(&cfg, &args[1..]).map_err(|error| io::Error::other(format!("{error:#}")))
}

fn run_backfill(cfg: config::Config, args: &[String]) -> io::Result<()> {
    backfill::run(&cfg, &args[1..]).map_err(|error| io::Error::other(format!("{error:#}")))
}

#[cfg(test)]
#[path = "tests/main.rs"]
mod tests;
