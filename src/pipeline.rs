use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::{
    clients, config::Config, history, judge, media, references, retry, score, video, youtube,
};

pub const STAGES: [&str; 11] = [
    "Image prompt",
    "Prepare image",
    "Image pool",
    "Download",
    "Reference",
    "Upload",
    "Audio drafts",
    "Review",
    "Final take",
    "Master + save",
    "Video",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Pending,
    Running,
    Ok,
    Failed,
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub file: PathBuf,
    pub seed: u32,
}

#[derive(Clone, Debug)]
pub struct Pool {
    pub images: Vec<Candidate>,
    pub audios: Vec<Candidate>,
}

#[derive(Clone, Copy, Debug)]
pub struct Selection {
    pub image: usize,
    pub audio: usize,
}

pub struct Outcome {
    pub audio: PathBuf,
    pub image: PathBuf,
    pub video: PathBuf,
    pub seed: u32,
    pub image_prompt: String,
}

#[derive(Clone, Debug)]
pub struct ReviewRequest {
    pub pool: Pool,
    /// Indices suggested by the cloud judge, if it ran and answered.
    pub suggestion: Option<Selection>,
    /// Best-scored pair when the judge did not answer.
    pub default: Selection,
    pub note: Option<String>,
}

pub enum Msg {
    Stage(usize, State),
    Log(String),
    Choose(ReviewRequest),
    Done(Result<Outcome, String>),
}

/// Starts the run and returns the channel the UI uses to answer the review.
pub fn spawn(cfg: Config, url: String, vibe: String, tx: Sender<Msg>) -> Sender<Selection> {
    let (decision_tx, decision_rx) = channel();
    thread::spawn(move || {
        let result =
            run(&cfg, &url, &vibe, &tx, &decision_rx).map_err(|error| format!("{error:#}"));
        let _ = tx.send(Msg::Done(result));
    });
    decision_tx
}

fn step<T>(tx: &Sender<Msg>, idx: usize, f: impl FnOnce() -> Result<T>) -> Result<T> {
    let _ = tx.send(Msg::Stage(idx, State::Running));
    match f() {
        Ok(value) => {
            let _ = tx.send(Msg::Stage(idx, State::Ok));
            Ok(value)
        }
        Err(err) => {
            let _ = tx.send(Msg::Stage(idx, State::Failed));
            let _ = tx.send(Msg::Log(format!("{}: {err:#}", STAGES[idx])));
            Err(err)
        }
    }
}

fn log(tx: &Sender<Msg>, text: impl Into<String>) {
    let _ = tx.send(Msg::Log(text.into()));
}

/// One line per stage transition, shared by the TUI log and the headless view.
pub fn stage_label(idx: usize, state: State) -> Option<String> {
    let name = STAGES.get(idx)?;
    let number = idx + 1;
    let total = STAGES.len();
    match state {
        State::Pending => None,
        State::Running => Some(format!("▶ {number}/{total} {name}")),
        State::Ok => Some(format!("✔ {number}/{total} {name}")),
        State::Failed => Some(format!("✖ {number}/{total} {name}")),
    }
}

fn run(
    cfg: &Config,
    url: &str,
    vibe: &str,
    tx: &Sender<Msg>,
    decisions: &Receiver<Selection>,
) -> Result<Outcome> {
    let count = cfg.candidates.max(1);
    let slug = slugify(vibe);
    let root = cfg.out_dir.join(&slug);
    let work = root.join("work");
    let candidates_dir = root.join("candidates");
    std::fs::create_dir_all(&work).context("creating the work directory")?;
    std::fs::create_dir_all(&candidates_dir).context("creating the candidates directory")?;
    let seed = random_seed();
    let image_seed = cfg.image_seed.unwrap_or_else(random_seed);
    let started_at = history::now_iso8601();
    let clock = Instant::now();

    log(tx, format!("working in {}", root.display()));
    log(
        tx,
        format!("audio seed {seed} · image seed {image_seed} · {count} candidates"),
    );
    media::preflight()?;
    video::preflight(cfg)?;

    let image_references =
        match references::images(&cfg.image_references, vibe, &cfg.image_reference_concept) {
            Ok(files) if !files.is_empty() => {
                log(
                    tx,
                    format!(
                        "references: {} images from {}",
                        files.len(),
                        cfg.image_references.display()
                    ),
                );
                files
            }
            Ok(files) => {
                if !cfg.image_references.as_os_str().is_empty() && files.is_empty() {
                    log(
                        tx,
                        format!(
                            "references: no images in {} (check the concept folder)",
                            cfg.image_references.display()
                        ),
                    );
                }
                files
            }
            Err(error) => {
                log(tx, format!("warning: references disabled: {error:#}"));
                Vec::new()
            }
        };

    // The LLM writes the thumbnail prompt while it still owns the GPU. It also
    // writes the YouTube copy and the audio arrangement here so no other stage
    // pays for a second load.
    let (image_prompt_text, youtube_draft, audio_arrangement) = step(tx, 0, || {
        media::ssh_run(cfg, &cfg.llm_start_cmd)?;
        clients::wait_llm(cfg, Duration::from_secs(180))?;
        let prompt = clients::ask_image_prompt(cfg, vibe)?;
        let youtube = match clients::ask_youtube(cfg, vibe) {
            Ok(draft) => Some(draft),
            Err(error) => {
                log(
                    tx,
                    format!("youtube: keeping the static template: {error:#}"),
                );
                None
            }
        };
        let arrangement = match clients::ask_audio_arrangement(cfg, vibe) {
            Ok(text) => Some(text),
            Err(error) => {
                log(
                    tx,
                    format!("arrangement: keeping the local template: {error:#}"),
                );
                None
            }
        };
        Ok((prompt, youtube, arrangement))
    })?;
    let mut image_prompt = retry::Prompt::new(image_prompt_text, cfg.image_negative_prompt.clone());
    let youtube_models = youtube::Models {
        image: &cfg.image_model,
        audio: &cfg.audio_model,
        creative: &cfg.creative_model,
    };
    let youtube = youtube::compose(vibe, youtube_draft.as_ref(), &youtube_models);
    log(tx, format!("image prompt: {}", image_prompt.positive));

    // Free the LLM, then bring up the image server.
    step(tx, 1, || {
        media::ssh_run(cfg, &cfg.llm_stop_cmd)?;
        media::ssh_run(cfg, &cfg.sd_start_cmd)?;
        clients::wait_sd(cfg, Duration::from_secs(180))
    })?;

    let image_started = std::time::Instant::now();
    let (image_scored, image_patches, image_rounds, image_reason) = step(tx, 2, || {
        image_pool(
            cfg,
            tx,
            &candidates_dir,
            &mut image_prompt,
            image_seed,
            count as usize,
            &image_references,
        )
    })?;
    let images: Vec<Candidate> = image_scored
        .iter()
        .map(|item| item.candidate.clone())
        .collect();
    log(
        tx,
        format!(
            "image pool: {} candidates in {:.1} min · {}",
            images.len(),
            image_started.elapsed().as_secs_f64() / 60.0,
            image_reason
        ),
    );

    let source = step(tx, 3, || media::download_audio(url, &work))?;
    log(tx, format!("source: {}", source.display()));

    let reference = step(tx, 4, || {
        if cfg.audio_reference.as_os_str().is_empty() {
            media::pick_reference(cfg, &source, &work)
        } else {
            media::prepare_reference(&cfg.audio_reference, &work, cfg.reference_sec)
        }
    })?;
    let reference_origin = if cfg.audio_reference.as_os_str().is_empty() {
        "source window"
    } else {
        "AUDIO_REFERENCE"
    };
    log(
        tx,
        format!(
            "reference ({reference_origin}): {} ({:.0} s)",
            reference.display(),
            media::duration_secs(&reference)?
        ),
    );

    let remote = step(tx, 5, || media::push_reference(cfg, &reference, &slug))?;
    log(tx, format!("remote reference: {remote}"));

    let mut audio_prompt = retry::Prompt::new(
        compose_audio_prompt(AudioBrief {
            vibe,
            style: &cfg.audio_style,
            arrangement: audio_arrangement.as_deref(),
            detail: &cfg.audio_detail,
        }),
        cfg.audio_negative_prompt.clone(),
    );
    log(tx, format!("prompt audio: {}", audio_prompt.positive));

    let drafts_result = step(tx, 6, || {
        // The timbre reference pushes ACE-Step past what fits next to the 4B,
        // so both the image model and the LLM go away before the audio stage.
        media::ssh_run(cfg, &cfg.sd_stop_cmd)?;
        media::ssh_run(cfg, &cfg.llm_stop_cmd)?;
        media::ssh_run(cfg, &cfg.audio_start_cmd)?;
        clients::wait_audio(cfg, Duration::from_secs(180))?;
        audio_pool(AudioPool {
            cfg,
            tx,
            dir: &candidates_dir,
            prompt: &mut audio_prompt,
            reference: &remote,
            seed,
            count: count as usize,
        })
    });
    let (audio_scored, audio_patches, audio_rounds, audio_reason) = match drafts_result {
        Ok(pool) => pool,
        Err(err) => {
            let _ = media::ssh_run(cfg, &cfg.audio_stop_cmd);
            return Err(err);
        }
    };
    let audios: Vec<Candidate> = audio_scored
        .iter()
        .map(|item| item.candidate.clone())
        .collect();
    let rounds = image_rounds.max(audio_rounds);
    let stop = format!("image: {image_reason}; audio: {audio_reason}");
    let mut patches = image_patches;
    patches.extend(audio_patches);

    // The audio server stays loaded while the review is open and the final
    // take runs, then it is released even if something fails.
    let pool = Pool {
        images: images.clone(),
        audios: audios.clone(),
    };
    let default = Selection {
        image: best_index(&image_scored).unwrap_or(0),
        audio: best_index(&audio_scored).unwrap_or(0),
    };
    let deterministic = findings_brief(&image_scored, &audio_scored);
    let (selection, judge_summary) = step(tx, 7, || {
        let mut suggestion = None;
        let mut summary = None;
        let mut note = None;
        if judge::enabled(cfg) {
            log(
                tx,
                format!(
                    "judge: reviewing thumbnails and clips with {}",
                    judge::describe(cfg)
                ),
            );
            let request = judge::Request {
                vibe,
                images: &images,
                audios: &audios,
                work: &work,
                findings: Some(&deterministic),
            };
            match judge::decide(cfg, request) {
                Ok(verdict) => {
                    let detail = format!(
                        "image {} · audio {} · score {}{} · {}",
                        verdict.image + 1,
                        verdict.audio + 1,
                        verdict.score,
                        verdict.must_fix_suffix(),
                        verdict.reason
                    );
                    log(tx, format!("judge: {detail}"));
                    suggestion = Some(Selection {
                        image: verdict.image,
                        audio: verdict.audio,
                    });
                    note = Some(detail);
                    summary = Some(verdict.summary());
                }
                Err(error) => log(
                    tx,
                    format!("judge: failed, keeping the manual pick: {error:#}"),
                ),
            }
        } else if cfg.judge_enabled {
            log(
                tx,
                format!("judge: enabled but no API key for {}", judge::describe(cfg)),
            );
        }
        let request = ReviewRequest {
            pool: pool.clone(),
            suggestion,
            default,
            note,
        };
        let _ = tx.send(Msg::Choose(request));
        let selection = decisions.recv().context("the review was cancelled")?;
        Ok((selection, summary))
    })?;
    let selected_image = images
        .get(selection.image)
        .context("the selected image does not exist")?;
    let selected_audio = audios
        .get(selection.audio)
        .context("the selected audio draft does not exist")?;
    log(
        tx,
        format!(
            "selected image {} and audio draft {}",
            selection.image + 1,
            selection.audio + 1
        ),
    );

    let final_result = step(tx, 8, || {
        let bytes = clients::generate_audio(
            cfg,
            clients::AudioRequest {
                prompt: &audio_prompt.positive,
                reference: &remote,
                seed: selected_audio.seed,
                duration_sec: cfg.duration_sec,
                steps: cfg.steps,
                sampler: &cfg.sampler,
            },
        )?;
        let take = work.join("take.wav");
        std::fs::write(&take, &bytes).context("saving the take")?;
        log(
            tx,
            format!(
                "final take: {} ({:.1} MB)",
                take.display(),
                bytes.len() as f64 / 1e6
            ),
        );
        Ok(take)
    });
    let stopped = media::ssh_run(cfg, &cfg.audio_stop_cmd);
    if let Err(err) = &stopped {
        log(
            tx,
            format!("warning: the audio server is still up: {err:#}"),
        );
    }
    let take = final_result?;

    let master = format!(
        "fade-in {} s, fade-out {} s, {} LUFS",
        cfg.fade_in_sec, cfg.fade_out_sec, cfg.loudness_lufs
    );

    let (final_audio, final_image) = step(tx, 9, || {
        let mastered = media::master_take(cfg, &take, &work)?;
        let mp3 = media::encode_mp3(&mastered, &work, 320)?;
        log(
            tx,
            format!(
                "master: {} ({:.0} s)",
                mp3.display(),
                media::duration_secs(&mp3)?
            ),
        );

        let final_audio = root.join("audio.mp3");
        std::fs::copy(&mp3, &final_audio)?;
        let final_image = root.join("thumbnail.png");
        std::fs::copy(&selected_image.file, &final_image)?;
        Ok((final_audio, final_image))
    })?;

    let video = step(tx, 10, || {
        let video::Rendered {
            file: video,
            renderer,
            particles,
        } = video::render(cfg, &final_image, &final_audio, &root, &mut |line| {
            log(tx, line)
        })?;
        let megabytes = std::fs::metadata(&video)
            .map(|meta| meta.len())
            .unwrap_or(0) as f64
            / 1e6;
        log(
            tx,
            format!("video: {} ({megabytes:.1} MB)", video.display()),
        );

        let image_candidates: Vec<_> = image_scored
            .iter()
            .map(|item| {
                serde_json::json!({
                    "file": item.candidate.file.display().to_string(),
                    "seed": item.candidate.seed,
                    "prompt": item.prompt,
                    "negative_prompt": item.negative,
                    "references": item
                        .references
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>(),
                })
            })
            .collect();
        let audio_candidates: Vec<_> = audios
            .iter()
            .map(|candidate| {
                serde_json::json!({
                    "file": candidate.file.display().to_string(),
                    "seed": candidate.seed,
                })
            })
            .collect();

        let youtube_file = root.join("youtube.txt");
        std::fs::write(&youtube_file, &youtube.description)
            .context("saving the YouTube description")?;
        log(
            tx,
            format!("youtube: {} ({})", youtube.title, youtube_file.display()),
        );

        let meta = serde_json::json!({
            "slug": slug,
            "source_url": url,
            "vibe": vibe,
            "image_prompt": &image_prompt.positive,
            "seed": selected_audio.seed,
            "image": {
                "size": format!("{}x{}", cfg.width, cfg.height),
                "steps": cfg.image_steps,
                "negative_prompt": &image_prompt.negative,
                "candidates": image_candidates,
                "selected": selection.image + 1,
                "file": final_image.display().to_string(),
            },
            "audio": {
                "model": cfg.audio_model,
                "prompt": &audio_prompt.positive,
                "negative_prompt": &audio_prompt.negative,
                "draft": {
                    "duration_sec": cfg.audio_draft_sec,
                    "steps": cfg.audio_draft_steps,
                    "sampler": cfg.audio_draft_sampler,
                    "candidates": audio_candidates,
                    "selected": selection.audio + 1,
                },
                "final": {
                    "duration_sec": cfg.duration_sec,
                    "steps": cfg.steps,
                    "guidance": cfg.guidance,
                    "sampler": cfg.sampler,
                    "seed": selected_audio.seed,
                },
                "master": master,
                "reference": remote,
                "file": final_audio.display().to_string(),
            },
            "video": {
                "file": video.display().to_string(),
                "size": format!("{}x{}", cfg.video_width, cfg.video_height),
                "renderer": renderer,
                "particles": particles,
            },
            "youtube": {
                "title": &youtube.title,
                "description": &youtube.description,
                "hashtags": &youtube.hashtags,
                "file": youtube_file.display().to_string(),
            },
            "retry": {
                "rounds": rounds,
                "image_rounds": image_rounds,
                "audio_rounds": audio_rounds,
                "stop": { "image": image_reason, "audio": audio_reason },
                "patches": patches,
            },
            "judge": judge_summary,
        });
        std::fs::write(root.join("meta.json"), serde_json::to_vec_pretty(&meta)?)?;
        Ok(video)
    })?;

    let seconds = clock.elapsed().as_secs_f64();
    record_history(
        cfg,
        tx,
        HistoryInput {
            root: &root,
            slug: &slug,
            vibe,
            started_at: &started_at,
            seconds,
            rounds,
            patches: &patches,
            stop: &stop,
        },
    );

    Ok(Outcome {
        audio: final_audio,
        image: final_image,
        video,
        seed: selected_audio.seed,
        image_prompt: image_prompt.positive,
    })
}

/// The TUI selects with 1-9, so the pool never grows past nine per side.
const MAX_POOL: usize = 9;

/// A candidate with its deterministic score, when scoring did not fail, and
/// the prompt that produced it. The prompt is per candidate because a retry
/// round can change it.
struct Scored {
    candidate: Candidate,
    side: Option<score::SideScore>,
    prompt: String,
    negative: String,
    references: Vec<PathBuf>,
}

fn best_index(pool: &[Scored]) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (index, item) in pool.iter().enumerate() {
        let Some(side) = &item.side else { continue };
        let better = best.is_none_or(|current: usize| {
            let current_score = pool[current]
                .side
                .as_ref()
                .map_or(f64::MIN, |value| value.score);
            side.score > current_score
        });
        if better {
            best = Some(index);
        }
    }
    best
}

/// Eight points of the 66-point audio side, in normalized terms.
fn audio_lost_enough(score: f64) -> bool {
    (100.0 - score) / 2.0 >= 8.0
}

fn log_best(tx: &Sender<Msg>, label: &str, pool: &[Scored]) {
    if let Some(index) = best_index(pool)
        && let Some(side) = &pool[index].side
    {
        let name = pool[index]
            .candidate
            .file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("?");
        log(tx, format!("{label} best: {:.1} ({name})", side.score));
    }
}

struct ImageRound<'a> {
    cfg: &'a Config,
    tx: &'a Sender<Msg>,
    dir: &'a Path,
    prompt: &'a retry::Prompt,
    seed: u32,
    start: usize,
    count: usize,
    references: &'a [PathBuf],
}

fn generate_images(round: ImageRound<'_>) -> Result<Vec<Scored>> {
    let mut scored = Vec::with_capacity(round.count);
    for offset in 0..round.count {
        let position = round.start + offset;
        let candidate_seed = round.seed.wrapping_add(position as u32);
        let bytes = clients::generate_image(
            round.cfg,
            &round.prompt.positive,
            &round.prompt.negative,
            candidate_seed,
            round.references,
        )?;
        let path = round.dir.join(format!("image-{:02}.png", position + 1));
        std::fs::write(&path, &bytes).context("saving an image candidate")?;
        let (width, height) = media::png_size(&path)?;
        if (width, height) != (round.cfg.width, round.cfg.height) {
            log(
                round.tx,
                format!(
                    "warning: candidate {} returned {width}x{height} instead of {}x{}",
                    position + 1,
                    round.cfg.width,
                    round.cfg.height
                ),
            );
        }
        log(
            round.tx,
            format!(
                "image candidate {}: {} (seed {candidate_seed}, {:.1} MB)",
                position + 1,
                path.display(),
                bytes.len() as f64 / 1e6
            ),
        );
        let candidate = Candidate {
            file: path,
            seed: candidate_seed,
        };
        let mut item = Scored {
            candidate,
            side: None,
            prompt: round.prompt.positive.clone(),
            negative: round.prompt.negative.clone(),
            references: round.references.to_vec(),
        };
        match score::gated_image_side(&item.candidate.file) {
            Ok((side, gates)) if gates.is_empty() => item.side = Some(side),
            Ok((_, gates)) => {
                let codes = gates
                    .iter()
                    .map(|gate| gate.code)
                    .collect::<Vec<_>>()
                    .join(",");
                log(
                    round.tx,
                    format!(
                        "warning: discarded {}: gate {codes}",
                        item.candidate.file.display()
                    ),
                );
            }
            Err(error) => log(
                round.tx,
                format!(
                    "warning: could not score {}: {error:#}",
                    item.candidate.file.display()
                ),
            ),
        }
        scored.push(item);
    }
    Ok(scored)
}

struct AudioRound<'a> {
    cfg: &'a Config,
    tx: &'a Sender<Msg>,
    dir: &'a Path,
    prompt: &'a retry::Prompt,
    reference: &'a str,
    seed: u32,
    start: usize,
    count: usize,
}

fn generate_drafts(round: AudioRound<'_>) -> Result<Vec<Scored>> {
    let mut scored = Vec::with_capacity(round.count);
    for offset in 0..round.count {
        let position = round.start + offset;
        let candidate_seed = round.seed.wrapping_add(position as u32);
        let bytes = clients::generate_audio(
            round.cfg,
            clients::AudioRequest {
                prompt: &round.prompt.positive,
                reference: round.reference,
                seed: candidate_seed,
                duration_sec: round.cfg.audio_draft_sec.max(10),
                steps: round.cfg.audio_draft_steps.max(1),
                sampler: &round.cfg.audio_draft_sampler,
            },
        )?;
        let path = round.dir.join(format!("audio-{:02}.wav", position + 1));
        std::fs::write(&path, &bytes).context("saving an audio draft")?;
        log(
            round.tx,
            format!(
                "audio draft {}: {} (seed {candidate_seed}, {:.0} s)",
                position + 1,
                path.display(),
                media::duration_secs(&path)?
            ),
        );
        let candidate = Candidate {
            file: path,
            seed: candidate_seed,
        };
        let mut item = Scored {
            candidate,
            side: None,
            prompt: round.prompt.positive.clone(),
            negative: round.prompt.negative.clone(),
            references: Vec::new(),
        };
        match score::gated_draft_side(
            &item.candidate.file,
            round.cfg.audio_draft_sec.max(10) as f64,
        ) {
            Ok((side, gates)) if gates.is_empty() => item.side = Some(side),
            Ok((_, gates)) => {
                let codes = gates
                    .iter()
                    .map(|gate| gate.code)
                    .collect::<Vec<_>>()
                    .join(",");
                log(
                    round.tx,
                    format!(
                        "warning: discarded {}: gate {codes}",
                        item.candidate.file.display()
                    ),
                );
            }
            Err(error) => log(
                round.tx,
                format!(
                    "warning: could not score {}: {error:#}",
                    item.candidate.file.display()
                ),
            ),
        }
        scored.push(item);
    }
    Ok(scored)
}

/// Generates the image pool and retries it while sd-server is still up. The
/// best candidate of the grown pool is kept; the review sees all of them.
fn image_pool(
    cfg: &Config,
    tx: &Sender<Msg>,
    dir: &Path,
    prompt: &mut retry::Prompt,
    seed: u32,
    count: usize,
    refs: &[PathBuf],
) -> Result<(Vec<Scored>, Vec<retry::Applied>, u32, String)> {
    let budget = cfg.score_rounds.min(retry::IMAGE_ROUNDS);
    let reference_count = cfg.image_reference_count as usize;
    let initial_refs = references::pick(refs, 0, reference_count);
    let mut pool = generate_images(ImageRound {
        cfg,
        tx,
        dir,
        prompt,
        seed,
        start: 0,
        count,
        references: &initial_refs,
    })?;
    let mut patches = Vec::new();
    let mut used = BTreeSet::new();
    let mut rounds = 0;
    let mut reason = if budget == 0 {
        "the image retry is disabled".to_string()
    } else {
        String::new()
    };
    log_best(tx, "image", &pool);

    for round in 1..=budget {
        let Some(best) = best_index(&pool) else {
            reason = "no image candidate could be scored".to_string();
            break;
        };
        let Some(side) = pool[best].side.as_ref() else {
            break;
        };
        if side.score >= score::TARGET_TECHNICAL {
            reason = format!("the best image scores {:.1}", side.score);
            break;
        }
        let Some((patch, finding)) = retry::choose(&side.findings, &used) else {
            reason = format!("no patchable finding at {:.1}", side.score);
            break;
        };
        if pool.len() >= MAX_POOL {
            reason = format!("the pool is full ({MAX_POOL})");
            break;
        }
        let applied = prompt.apply(patch, finding.code);
        used.insert(patch.key);
        let add = count.min(MAX_POOL - pool.len());
        log(
            tx,
            format!(
                "retry image {round}/{budget}: {} {:+.1}, {add} new candidate(s)",
                finding.code, finding.delta
            ),
        );
        let retry_refs =
            references::pick(refs, (round as usize) * reference_count, reference_count);
        pool.extend(generate_images(ImageRound {
            cfg,
            tx,
            dir,
            prompt,
            seed,
            start: pool.len(),
            count: add,
            references: &retry_refs,
        })?);
        patches.push(applied);
        rounds = round;
        log_best(tx, "image", &pool);
    }

    if reason.is_empty() {
        reason = match best_index(&pool).and_then(|index| pool[index].side.as_ref()) {
            Some(side) if side.score >= score::TARGET_TECHNICAL => {
                format!("the best image scores {:.1}", side.score)
            }
            Some(side) => format!("the image budget is exhausted at {:.1}", side.score),
            None => "no image candidate could be scored".to_string(),
        };
    }
    Ok((pool, patches, rounds, reason))
}

struct AudioPool<'a> {
    cfg: &'a Config,
    tx: &'a Sender<Msg>,
    dir: &'a Path,
    prompt: &'a mut retry::Prompt,
    reference: &'a str,
    seed: u32,
    count: usize,
}

/// Generates the drafts and runs at most one audio round when the audio lost
/// eight points or more of its 50. The image side does not gate it.
fn audio_pool(request: AudioPool<'_>) -> Result<(Vec<Scored>, Vec<retry::Applied>, u32, String)> {
    let AudioPool {
        cfg,
        tx,
        dir,
        prompt,
        reference,
        seed,
        count,
    } = request;
    let budget = cfg.score_rounds.min(retry::AUDIO_ROUNDS);
    let mut pool = generate_drafts(AudioRound {
        cfg,
        tx,
        dir,
        prompt,
        reference,
        seed,
        start: 0,
        count,
    })?;
    let mut patches = Vec::new();
    let mut used = BTreeSet::new();
    let mut rounds = 0;
    let mut reason = if budget == 0 {
        "the audio retry is disabled".to_string()
    } else {
        String::new()
    };
    log_best(tx, "audio", &pool);

    for round in 1..=budget {
        let Some(best) = best_index(&pool) else {
            reason = "no audio candidate could be scored".to_string();
            break;
        };
        let Some(side) = pool[best].side.as_ref() else {
            break;
        };
        if side.score >= score::TARGET_TECHNICAL {
            reason = format!("the best audio scores {:.1}", side.score);
            break;
        }
        let lost = (100.0 - side.score) / 2.0;
        if !audio_lost_enough(side.score) {
            reason = format!("the audio lost {lost:.1} points, under the 8-point rule");
            break;
        }
        let Some((patch, finding)) = retry::choose(&side.findings, &used) else {
            reason = format!("no patchable finding at {:.1}", side.score);
            break;
        };
        if pool.len() >= MAX_POOL {
            reason = format!("the pool is full ({MAX_POOL})");
            break;
        }
        let applied = prompt.apply(patch, finding.code);
        used.insert(patch.key);
        let add = count.min(MAX_POOL - pool.len());
        log(
            tx,
            format!(
                "retry audio {round}/{budget}: {} {:+.1}, {add} new candidate(s)",
                finding.code, finding.delta
            ),
        );
        pool.extend(generate_drafts(AudioRound {
            cfg,
            tx,
            dir,
            prompt,
            reference,
            seed,
            start: pool.len(),
            count: add,
        })?);
        patches.push(applied);
        rounds = round;
        log_best(tx, "audio", &pool);
    }

    if reason.is_empty() {
        reason = match best_index(&pool).and_then(|index| pool[index].side.as_ref()) {
            Some(side) if side.score >= score::TARGET_TECHNICAL => {
                format!("the best audio scores {:.1}", side.score)
            }
            Some(side) => format!("the audio budget is exhausted at {:.1}", side.score),
            None => "no audio candidate could be scored".to_string(),
        };
    }
    Ok((pool, patches, rounds, reason))
}

/// Compact text for the judge so it does not repeat what the rubric measured.
/// The worst findings across both sides survive the 16-line cap.
fn findings_brief(images: &[Scored], audios: &[Scored]) -> String {
    let mut lines: Vec<(f64, String)> = Vec::new();
    for (label, pool) in [("image", images), ("audio", audios)] {
        for (index, item) in pool.iter().enumerate() {
            let Some(side) = &item.side else { continue };
            for finding in &side.findings {
                if finding.delta > -retry::MIN_LOSS {
                    continue;
                }
                lines.push((
                    finding.delta,
                    format!(
                        "{} {}: {} {} (lost {:.1})",
                        label,
                        index + 1,
                        finding.code,
                        finding.message,
                        finding.delta
                    ),
                ));
            }
        }
    }
    lines.sort_by(|left, right| left.0.total_cmp(&right.0));
    lines.truncate(16);
    if lines.is_empty() {
        "none".to_string()
    } else {
        lines
            .into_iter()
            .map(|(_, line)| line)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

struct HistoryInput<'a> {
    root: &'a Path,
    slug: &'a str,
    vibe: &'a str,
    started_at: &'a str,
    seconds: f64,
    rounds: u32,
    patches: &'a [retry::Applied],
    stop: &'a str,
}

/// Scores the finished run and appends it to the local database. Failures are
/// logged and never abort the run, the same way the judge fails open.
fn record_history(cfg: &Config, tx: &Sender<Msg>, input: HistoryInput<'_>) {
    log(tx, "history: scoring the run for the local database");
    let result = (|| -> Result<(i64, score::RunScore, String)> {
        let score = score::score_run(input.root)?;
        score::write_json(input.root, &score)?;
        let outcome = if score.accepted {
            "accepted"
        } else {
            "accepted_by_limit"
        }
        .to_string();
        let reason = if score.accepted {
            None
        } else {
            Some(format!(
                "{}; technical {:.1}, total {:.1}",
                input.stop, score.technical, score.total
            ))
        };
        let record = history::RunRecord {
            db: &cfg.db_path,
            dir: input.root,
            slug: input.slug,
            vibe: input.vibe,
            started_at: input.started_at,
            seconds: input.seconds,
            rounds: input.rounds,
            patches: input.patches,
            outcome: &outcome,
            outcome_reason: reason.as_deref(),
        };
        let id = history::insert(&record, &score)?;
        Ok((id, score, outcome))
    })();
    match result {
        Ok((id, score, outcome)) => {
            let editorial = score
                .editorial
                .map(|value| format!("{value:.1}"))
                .unwrap_or_else(|| "-".to_string());
            log(
                tx,
                format!(
                    "history: run {id} · technical {:.1} · editorial {editorial} · total {:.1} · {outcome}",
                    score.technical, score.total
                ),
            );
            if !score.accepted {
                log(tx, format!("history: accepted by limit: {}", input.stop));
            }
            let worst = score
                .final_image
                .findings
                .iter()
                .chain(score.final_audio.findings.iter())
                .min_by(|a, b| a.delta.total_cmp(&b.delta));
            if let Some(finding) = worst {
                log(
                    tx,
                    format!(
                        "history: worst finding {} {:+.1} ({})",
                        finding.code, finding.delta, finding.message
                    ),
                );
            }
        }
        Err(error) => log(
            tx,
            format!("warning: the history was not written: {error:#}"),
        ),
    }
}

fn random_seed() -> u32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(1);
    nanos.max(1)
}

pub fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in text.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
        if out.len() >= 40 {
            break;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "take".to_string()
    } else {
        trimmed
    }
}

/// Words that switch the arrangement to the calm variant. The vibe carries the
/// mood, so a chill brief must not get the driving drums and
/// maximum-intensity climax of the default arrangement. Substring match on
/// purpose: relax covers relaxing, sleep covers sleepy.
const CALM_WORDS: &[&str] = &[
    "ambient", "calm", "chill", "cozy", "dreamy", "gentle", "lullaby", "mellow", "pastoral",
    "peaceful", "quiet", "relax", "serene", "sleep", "slow", "soft", "tranquil",
];

fn vibe_is_calm(vibe: &str) -> bool {
    let haystack = vibe.to_ascii_lowercase();
    CALM_WORDS.iter().any(|word| haystack.contains(word))
}

/// Named brief for the audio prompt: over three params, so they travel as one
/// object. `arrangement` is the LLM-written sectional body from stage 0, which
/// covers arbitrary vibes; when that call failed, the local keyword template
/// below stands in as the offline fallback.
pub struct AudioBrief<'a> {
    pub vibe: &'a str,
    pub style: &'a str,
    pub arrangement: Option<&'a str>,
    pub detail: &'a str,
}

/// The channel medium is a rule: the vibe adds mood, the style is always there.
/// The skeleton is fixed (style, tempo, lead, rhythm, layers, harmony, dynamics,
/// production, structure); the content comes from the LLM arrangement when
/// stage 0 produced one, else from the keyword template. `detail` holds run-
/// specific extras (BPM, instruments) and is appended verbatim; it stays empty
/// unless the run needs it. Instrumental only, always.
fn keyword_template(vibe: &str, style: &str) -> String {
    if vibe_is_calm(vibe) {
        format!(
            "STYLE\n\
         Instrumental {style} music in the world of {vibe}: a tender, soothing piece \
         with a soft musical identity and a calm, breathing flow, composed as a continuous \
         gentle journey rather than a conventional pop song. Instrumental only. No vocals. An \
         original composition with its own melodies, riffs, chord progressions and musical \
         identity.\n\n\
         TEMPO AND RHYTHM\n\
         A slow tempo and relaxed meter that fit the mood of {vibe}, with a soft, unhurried \
         pulse throughout the composition. Light, rounded percussion with gentle taps, soft \
         shaker textures and warm low pulses, never a driving kick. The rhythm should feel \
         loose, breathing and full of calm.\n\n\
         LEAD\n\
         One expressive lead voice as the primary melodic instrument, with a soft, warm tone, \
         gentle sustain and lyrical phrasing. Build the musical identity around a simple, \
         memorable melodic motif with delicate variations and tender call-and-response phrases. \
         Soft, expressive and unmistakably melodic, with occasional harmonized lines and gentle \
         echoes between phrases.\n\n\
         RHYTHM SECTION\n\
         A soft, warm bass foundation with long sustained notes that loosely follow the gentle \
         pulse, cradling the low end without driving it. Rhythmic chords alternate between \
         sustained harmonic support and slow, rocking patterns.\n\n\
         LAYERS\n\
         Supporting orchestral and synthesizer layers that reinforce the atmosphere without \
         overpowering the lead: warm pads, soft bell-like tones and airy keys, with gentle \
         melodic responses, slow swells and dreamy accents.\n\n\
         HARMONY\n\
         Soft melodic hooks, gentle harmonic movement, mild tension and a warm, comforting \
         resolution, balancing the emotions of {vibe} with tender, expressive melodies. Simple \
         motifs, flowing sequences and carefully placed harmonic changes.\n\n\
         ARRANGEMENT\n\
         Introduce the central motif early over near-silence. Slowly add delicate layers and \
         vary the material so it never turns monotonous. Contrast slightly fuller passages \
         against sparse, intimate ones, with soft swells and unhurried transitions.\n\n\
         PRODUCTION\n\
         Professional-quality modern soundtrack production: clear separation between instruments, \
         soft percussion, warm low end, articulate lead, wide stereo imaging and controlled, \
         detailed layering. Tasteful room ambience and gentle reverb for depth and calm. A \
         polished, intimate, mellow sound.\n\n\
         STRUCTURE\n\
         [Intro] A short, quiet opening with a sparse melodic motif and immediate calm. \
         [Theme A] The central melodic motif over the gentle pulse, establishing the identity. \
         [Theme A variation] The motif repeated with soft harmonies and more delicate variations. \
         [Theme B] A contrasting passage with different harmony and slightly deeper feeling. \
         [Development] The main ideas gently developed through flowing variations and layered \
         harmonies. [Break] A near-silent passage for contrast and rest, keeping the calm. \
         [Climax] The main motif back at its warmest and fullest, still gentle. [Finale] A soft, \
         memorable conclusion that resolves into stillness. [End]"
        )
    } else {
        format!(
            "STYLE\n\
         Instrumental {style} music in the world of {vibe}: an engaging, emotionally charged piece \
         with a strong musical identity and constant forward momentum, composed as a continuous \
         musical journey rather than a conventional pop song. Instrumental only. No vocals. An \
         original composition with its own melodies, riffs, chord progressions and musical \
         identity.\n\n\
         TEMPO AND RHYTHM\n\
         A clear tempo and meter that fit the mood of {vibe}, with a driving rhythmic pulse \
         throughout the composition. Tight, punchy drums with powerful kick drum patterns, sharp \
         snare hits, energetic cymbal crashes and active hi-hat work. The rhythm should feel \
         precise, propulsive and full of momentum.\n\n\
         LEAD\n\
         One expressive lead voice as the primary melodic instrument, with a bright, defined tone, \
         strong sustain and articulate phrasing. Build the musical identity around a memorable, \
         repetitive melodic motif with variations, call-and-response phrases and increasingly \
         elaborate development. Confident, expressive and unmistakably melodic, with occasional \
         harmonized lines, layered counterpoint and short fills between phrases.\n\n\
         RHYTHM SECTION\n\
         A powerful bass foundation tightly locked to the kick drum, driving the low end with an \
         active line that reinforces the rhythmic patterns while adding occasional melodic \
         movement. Rhythmic chords alternate between sustained harmonic support and fast, \
         repetitive patterns.\n\n\
         LAYERS\n\
         Supporting orchestral and synthesizer layers that reinforce the atmosphere without \
         overpowering the lead: dramatic strings, brass-like accents, stabs and atmospheric keys, \
         with short melodic responses, dramatic transitions and cinematic accents.\n\n\
         HARMONY\n\
         Strong melodic hooks, dramatic harmonic movement, tension and triumphant resolution, \
         balancing the emotions of {vibe} with memorable, expressive melodies. Strong motifs, \
         energetic sequences and carefully placed harmonic changes.\n\n\
         ARRANGEMENT\n\
         Introduce the central motif early. Gradually add layers, increase complexity and vary the \
         material so it never turns monotonous. Contrast reduced passages against powerful \
         full-band returns, with dramatic transitions, fills and tightly synchronized accents.\n\n\
         PRODUCTION\n\
         Professional-quality modern soundtrack production: clear separation between instruments, \
         punchy drums, tight low end, articulate lead, wide stereo imaging and controlled, detailed \
         layering. Tasteful room ambience and moderate reverb for depth without losing clarity or \
         rhythmic precision. A polished, energetic, full-band sound.\n\n\
         STRUCTURE\n\
         [Intro] A short, energetic opening with a recognizable rhythmic motif and immediate \
         momentum. [Theme A] The central melodic motif over the driving rhythm section, \
         establishing the identity. [Theme A variation] The motif repeated with harmonies, accents \
         and more elaborate variations. [Theme B] A contrasting passage with different harmony, \
         more tension and more expressive phrasing. [Development] The main ideas developed through \
         faster runs, rhythmic variations and layered harmonies. [Break] Briefly reduced \
         instrumentation for contrast and anticipation, keeping the rhythmic identity. [Climax] The \
         main motif back with the full band at maximum intensity. [Finale] A powerful, memorable \
         conclusion with a decisive resolution. [End]"
        )
    }
}

pub fn compose_audio_prompt(brief: AudioBrief<'_>) -> String {
    let AudioBrief {
        vibe,
        style,
        arrangement,
        detail,
    } = brief;
    let body = match arrangement {
        Some(text) => text.trim().to_string(),
        None => keyword_template(vibe, style),
    };
    let base = format!("{vibe}, {style} instrumental track.\n\n{body}");
    let detail = detail.trim();
    if detail.is_empty() {
        base
    } else {
        format!("{base}\n\nDETAIL\n{detail}")
    }
}

#[cfg(test)]
#[path = "tests/pipeline.rs"]
mod tests;
