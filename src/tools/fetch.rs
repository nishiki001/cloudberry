//! Downloading and verifying the helper binaries.
use super::{deno_triple, exe_name, managed_dir, managed_path, ytdlp_asset};
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;
use std::time::Duration;

const YTDLP_BASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";
const DENO_BASE: &str = "https://github.com/denoland/deno/releases/latest/download";

/// What the first-run dialog shows.
#[derive(Debug, Clone)]
pub struct Progress {
    pub tool: &'static str,
    pub done: u64,
    pub total: Option<u64>,
}

impl Progress {
    pub fn text(&self) -> String {
        let mb = |b: u64| format!("{:.0} MB", b as f64 / 1_048_576.0);
        match self.total {
            Some(t) => format!("Downloading {}… {} of {}", self.tool, mb(self.done), mb(t)),
            None => format!("Downloading {}… {}", self.tool, mb(self.done)),
        }
    }
}

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent(concat!("cloudberry/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .build()?)
}

/// The hash for `file` in a `sha256sum`-style listing ("<hex>  <name>" lines, or a bare hash).
pub fn find_sum(listing: &str, file: &str) -> Option<String> {
    let mut bare = None;
    for line in listing.lines() {
        // PowerShell Get-FileHash output: "Hash      : ABC…"
        if let Some(h) = line
            .trim()
            .strip_prefix("Hash")
            .and_then(|r| r.trim_start().strip_prefix(':'))
        {
            bare = Some(h.trim().to_ascii_lowercase());
            continue;
        }
        let mut it = line.split_whitespace();
        let (Some(h), name) = (it.next(), it.next()) else {
            continue;
        };
        if h.len() != 64 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        match name.map(|n| n.trim_start_matches('*')) {
            Some(n) if n == file => return Some(h.to_ascii_lowercase()),
            None => bare = Some(h.to_ascii_lowercase()),
            _ => {}
        }
    }
    bare
}

fn get_text(c: &reqwest::blocking::Client, url: &str) -> Result<String> {
    Ok(c.get(url).send()?.error_for_status()?.text()?)
}

/// Stream `url` into `dest`, hashing and reporting progress; returns the SHA-256 (hex).
fn get_to_file(
    c: &reqwest::blocking::Client,
    url: &str,
    tool: &'static str,
    dest: &Path,
    report: &dyn Fn(Progress),
) -> Result<String> {
    use std::io::Write;
    let mut resp = c.get(url).send()?.error_for_status()?;
    let total = resp.content_length();
    std::fs::create_dir_all(managed_dir())?;
    let mut file = std::fs::File::create(dest)?;
    let (mut done, mut hasher) = (0usize, Sha256::new());
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = resp.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        file.write_all(&chunk[..n])?;
        hasher.update(&chunk[..n]);
        done += n;
        // about once per MB (and at the end), not per chunk
        if done / (1 << 20) != (done - n) / (1 << 20) || Some(done as u64) == total {
            report(Progress {
                tool,
                done: done as u64,
                total,
            });
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

fn make_executable(p: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755))?;
    }
    let _ = p;
    Ok(())
}

/// Finish an install: executable bit, then an atomic rename onto the final name.
fn finish(part: &Path, name: &str) -> Result<()> {
    make_executable(part)?;
    std::fs::rename(part, managed_dir().join(name))?;
    Ok(())
}

fn fetch_ytdlp(c: &reqwest::blocking::Client, report: &dyn Fn(Progress)) -> Result<()> {
    let asset = ytdlp_asset().context("no yt-dlp build for this platform")?;
    let sums = get_text(c, &format!("{YTDLP_BASE}/SHA2-256SUMS"))?;
    let want = find_sum(&sums, asset).context("yt-dlp checksum not published")?;
    let part = managed_dir().join("yt-dlp.part");
    let got = get_to_file(c, &format!("{YTDLP_BASE}/{asset}"), "yt-dlp", &part, report)?;
    if got != want {
        let _ = std::fs::remove_file(&part);
        bail!("yt-dlp checksum mismatch (got {got}, expected {want})");
    }
    finish(&part, &exe_name("yt-dlp"))
}

/// Copy the entry `name` of the zip at `zip_path` to `dest` (exact name only: no path traversal).
fn unzip_entry(zip_path: &Path, name: &str, dest: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(zip_path)?)?;
    let mut entry = zip
        .by_name(name)
        .with_context(|| format!("no {name} in the zip"))?;
    let mut out = std::fs::File::create(dest)?;
    std::io::copy(&mut entry, &mut out)?;
    Ok(())
}

fn fetch_deno(c: &reqwest::blocking::Client, report: &dyn Fn(Progress)) -> Result<()> {
    let triple = deno_triple().context("no deno build for this platform")?;
    let file = format!("deno-{triple}.zip");
    let url = format!("{DENO_BASE}/{file}");
    let sums = get_text(c, &format!("{url}.sha256sum"))?;
    let want = find_sum(&sums, &file).context("deno checksum not published")?;
    let zip_part = managed_dir().join("deno.zip.part");
    let got = get_to_file(c, &url, "deno", &zip_part, report)?;
    if got != want {
        let _ = std::fs::remove_file(&zip_part);
        bail!("deno checksum mismatch (got {got}, expected {want})");
    }
    let part = managed_dir().join("deno.part");
    let res = unzip_entry(&zip_part, &exe_name("deno"), &part);
    let _ = std::fs::remove_file(&zip_part);
    res?;
    finish(&part, &exe_name("deno"))
}

/// Download what is missing or does not start (blocking; call off the UI thread).
pub fn ensure(report: &dyn Fn(Progress)) -> Result<()> {
    let c = client()?;
    for tool in ["yt-dlp", "deno"] {
        let ok = managed_path(tool)
            .and_then(|p| super::version_of(&p))
            .is_some();
        if ok {
            continue;
        }
        match tool {
            "yt-dlp" => fetch_ytdlp(&c, report),
            _ => fetch_deno(&c, report),
        }
        .with_context(|| format!("could not download {tool}"))?;
    }
    Ok(())
}

/// `yt-dlp -U` on the managed copy; returns its report. Records the check time.
static UPDATING: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn update_ytdlp() -> Result<String> {
    // the daily check and "Update now" must not run `yt-dlp -U` on the binary at once
    let _guard = UPDATING.lock().unwrap_or_else(|e| e.into_inner());
    let p = managed_path("yt-dlp").context("yt-dlp is not installed (managed)")?;
    let out = std::process::Command::new(&p)
        .arg("-U")
        .stdin(std::process::Stdio::null())
        .output()?;
    let _ = std::fs::write(managed_dir().join("last-update-check"), now().to_string());
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() {
        bail!(
            "yt-dlp -U failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(text)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// True when the last update check is older than a day (or never happened).
pub fn update_due() -> bool {
    let last = std::fs::read_to_string(managed_dir().join("last-update-check"))
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);
    now().saturating_sub(last) > 24 * 3600
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn sums_listing_and_bare_hash() {
        let list = format!("{H}  yt-dlp_linux\n{}  yt-dlp.exe\n", "a".repeat(64));
        assert_eq!(find_sum(&list, "yt-dlp_linux").as_deref(), Some(H));
        assert_eq!(find_sum(&list, "yt-dlp_macos"), None);
        assert_eq!(find_sum(&format!("{H}\n"), "anything").as_deref(), Some(H));
        assert_eq!(find_sum("not a hash  file", "file"), None);
    }

    #[test]
    fn unzip_exact_entry_to_a_part_file() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("cb-unzip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let zp = dir.join("t.zip");
        let mut w = zip::ZipWriter::new(std::fs::File::create(&zp).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        w.start_file("deno", o).unwrap();
        w.write_all(b"binary").unwrap();
        w.start_file("../evil", o).unwrap();
        w.write_all(b"x").unwrap();
        w.finish().unwrap();
        unzip_entry(&zp, "deno", &dir.join("deno.part")).unwrap();
        assert_eq!(std::fs::read(dir.join("deno.part")).unwrap(), b"binary");
        assert!(unzip_entry(&zp, "other", &dir.join("o")).is_err());
        assert!(!dir.parent().unwrap().join("evil").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn powershell_hash_format() {
        assert_eq!(
            find_sum(&format!("Hash      : {}\n", H.to_uppercase()), "x").as_deref(),
            Some(H)
        );
    }

    #[test]
    fn progress_text() {
        let p = Progress {
            tool: "deno",
            done: 3 << 20,
            total: Some(40 << 20),
        };
        assert_eq!(p.text(), "Downloading deno… 3 MB of 40 MB");
    }
}
