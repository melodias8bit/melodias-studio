//! Helpers shared by the reference library builder: HTTP, hashing, paths,
//! ImageMagick, zip extraction and the small HTML scans used by OpenGameArt.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/126.0 Safari/537.36 melodias-studio";

pub fn http_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(10))
        .user_agent(USER_AGENT)
        .build()
        .context("creating the HTTP client")
}

/// One GET with retries on 429 and 5xx. `label` is what errors mention, so a
/// URL that carries a key (Pixabay) never lands in a log.
pub struct HttpRequest<'a> {
    pub client: &'a reqwest::blocking::Client,
    pub url: &'a str,
    pub label: &'a str,
    pub max_bytes: u64,
}

pub fn http_get(request: HttpRequest<'_>) -> Result<Vec<u8>> {
    let mut last = String::new();
    for attempt in 1..=3u32 {
        match request.client.get(request.url).send() {
            Ok(response) if response.status().is_success() => {
                if let Some(length) = response.content_length()
                    && length > request.max_bytes
                {
                    bail!(
                        "{} is larger than {} bytes",
                        request.label,
                        request.max_bytes
                    );
                }
                let bytes = response
                    .bytes()
                    .with_context(|| format!("reading {}", request.label))?;
                if bytes.len() as u64 > request.max_bytes {
                    bail!(
                        "{} is larger than {} bytes",
                        request.label,
                        request.max_bytes
                    );
                }
                return Ok(bytes.to_vec());
            }
            Ok(response)
                if response.status().as_u16() == 429 || response.status().is_server_error() =>
            {
                last = format!("HTTP {}", response.status());
                eprintln!("warning: {} answered {}, retrying", request.label, last);
                sleep(Duration::from_secs(u64::from(attempt) * 2));
            }
            Ok(response) => bail!("{} answered HTTP {}", request.label, response.status()),
            Err(error) => {
                last = error.to_string();
                if attempt < 3 {
                    eprintln!("warning: {} failed, retrying", request.label);
                    sleep(Duration::from_secs(u64::from(attempt) * 2));
                }
            }
        }
    }
    bail!("{} did not answer after 3 attempts ({last})", request.label)
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn url_extension(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((_, extension)) => extension.to_ascii_lowercase(),
        None => String::new(),
    }
}

pub fn is_image_extension(extension: &str) -> bool {
    matches!(extension, "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp")
}

pub fn valid_concept(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

pub fn valid_prefix(name: &str) -> bool {
    valid_concept(name)
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

pub fn expand_tilde(value: &str) -> PathBuf {
    if value == "~" {
        return home().unwrap_or_else(|| PathBuf::from(value));
    }
    if let Some(rest) = value.strip_prefix("~/")
        && let Some(home) = home()
    {
        return home.join(rest);
    }
    PathBuf::from(value)
}

pub fn default_root() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home().map(|home| home.join(".local/share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"));
    base.join("melodias-studio/references")
}

pub fn now_iso() -> String {
    if let Ok(output) = Command::new("date")
        .arg("-u")
        .arg("+%Y-%m-%dT%H:%M:%SZ")
        .output()
        && output.status.success()
    {
        return String::from_utf8_lossy(&output.stdout).trim().to_string();
    }
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_default()
}

fn which(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|directory| directory.join(name).is_file())
    })
}

pub fn magick_binary() -> Result<&'static str> {
    if which("magick") {
        return Ok("magick");
    }
    if which("convert") {
        return Ok("convert");
    }
    bail!("missing ImageMagick (magick) on PATH")
}

/// Normalizes one image to PNG with a longest side of `size`, never upscaling.
/// The PNG date chunks are excluded so the same source always hashes the same.
pub fn normalize_png(src: &Path, dest: &Path, size: u32) -> Result<()> {
    let magick = magick_binary()?;
    let status = Command::new(magick)
        .arg(format!("{}[0]", src.display()))
        .args(["-auto-orient", "-resize"])
        .arg(format!("{size}x{size}>"))
        .args([
            "-strip",
            "-define",
            "png:exclude-chunks=date,time",
            &format!("PNG32:{}", dest.display()),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("running {magick} on {}", src.display()))?;
    if !status.success() {
        bail!("{magick} could not read {}", src.display());
    }
    Ok(())
}

pub fn temp_dir(label: &str) -> Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
        "melodias-refs-{label}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&path).with_context(|| format!("creating {}", path.display()))?;
    Ok(path)
}

pub struct ExtractRequest<'a> {
    pub archive: &'a Path,
    pub dest: &'a Path,
    pub limit: usize,
    pub max_bytes: u64,
}

pub struct Scratch {
    path: PathBuf,
}

impl Scratch {
    pub fn new(label: &str) -> Result<Self> {
        Ok(Self {
            path: temp_dir(label)?,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub struct Extracted {
    pub path: PathBuf,
    pub name: String,
}

/// Extracts the image members of a zip, capped by count and by uncompressed
/// size, and returns their paths and original names in archive order.
pub fn extract_images(request: ExtractRequest<'_>) -> Result<Vec<Extracted>> {
    fs::create_dir_all(request.dest)
        .with_context(|| format!("creating {}", request.dest.display()))?;
    let file = File::open(request.archive)
        .with_context(|| format!("reading {}", request.archive.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("opening {} as a zip", request.archive.display()))?;
    let mut out = Vec::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        if out.len() >= request.limit {
            break;
        }
        let mut entry = archive
            .by_index(index)
            .with_context(|| format!("reading member {index} of {}", request.archive.display()))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry
            .name()
            .with_context(|| format!("reading member {index} name"))?
            .to_string();
        if name.starts_with("__MACOSX/") || name.starts_with("._") || name.contains("/._") {
            continue;
        }
        let extension = name
            .rsplit_once('.')
            .map(|(_, extension)| extension.to_ascii_lowercase())
            .unwrap_or_default();
        if !is_image_extension(&extension) {
            continue;
        }
        total = total.saturating_add(entry.size());
        if total > request.max_bytes {
            bail!(
                "{} expands to more than {} bytes",
                request.archive.display(),
                request.max_bytes
            );
        }
        let target = request.dest.join(format!("member-{index}.{extension}"));
        let mut sink =
            File::create(&target).with_context(|| format!("creating {}", target.display()))?;
        std::io::copy(&mut entry, &mut sink).with_context(|| format!("extracting {name}"))?;
        sink.flush().ok();
        out.push(Extracted { path: target, name });
    }
    Ok(out)
}

pub fn find_all<'a>(text: &'a str, needle: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(index) = rest.find(needle) {
        let after = &rest[index + needle.len()..];
        out.push(after);
        rest = after;
    }
    out
}

pub fn take_until<'a>(text: &'a str, stops: &[char]) -> &'a str {
    text.split(|character| stops.contains(&character))
        .next()
        .unwrap_or("")
}

pub fn decode_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

#[cfg(test)]
#[path = "../tests/refs_util.rs"]
mod tests;
