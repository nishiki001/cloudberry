//! `auth setup|check|logout`.
use crate::cli::AuthCmd;
use crate::config::{self, Config};
use crate::core::api::client::Client;
use crate::deps;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Stdio;

pub fn run(cmd: AuthCmd) -> Result<i32> {
    match cmd {
        AuthCmd::Setup { browser } => setup(browser),
        AuthCmd::Import { file } => {
            let n = import_cookie_file(&file)?;
            println!("imported {n} YouTube/Google cookies");
            check()
        }
        AuthCmd::Check => check(),
        AuthCmd::Logout => logout(),
    }
}

fn restrict(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn is_google_domain(d: &str) -> bool {
    let d = d.trim_start_matches("#HttpOnly_").trim_start_matches('.');
    ["youtube.com", "google.com"]
        .iter()
        .any(|b| d == *b || d.ends_with(&format!(".{b}")))
}

/// Keep only the header and YouTube/Google cookie lines of a Netscape jar.
pub fn filter_jar(text: &str) -> String {
    let mut out = String::from("# Netscape HTTP Cookie File\n");
    for line in text.lines() {
        let dom = line.split('\t').next().unwrap_or("");
        if line.split('\t').count() >= 7 && is_google_domain(dom) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn create_private(path: &Path) -> Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    Ok(o.open(path)?)
}

/// Export cookies through yt-dlp (we never read browser profiles ourselves). yt-dlp dumps the
/// whole jar into a private temp file; only YouTube/Google lines are kept in cookies.txt.
pub fn export_cookies(browser: &str) -> Result<()> {
    use std::io::Write;
    let found = deps::locate();
    let ytdlp = deps::path_of(&found, "yt-dlp").context("yt-dlp not found (run `doctor`)")?;
    let out = config::cookies_path();
    let dir = out.parent().unwrap();
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join("cookies.full.tmp");
    create_private(&tmp)?.write_all(b"# Netscape HTTP Cookie File\n")?;
    let status = std::process::Command::new(ytdlp)
        .args(["--cookies-from-browser", browser, "--cookies"])
        .arg(&tmp)
        .args([
            "--skip-download",
            "--no-warnings",
            "--quiet",
            "https://www.youtube.com/watch?v=jNQXAC9IVRw",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let result = (|| -> Result<()> {
        let status = status.context("run yt-dlp")?;
        anyhow::ensure!(
            status.success(),
            "yt-dlp failed (exit {status}); is {browser} installed and logged in?"
        );
        let filtered = filter_jar(&std::fs::read_to_string(&tmp).context("read exported cookies")?);
        anyhow::ensure!(
            filtered.lines().count() > 1,
            "no YouTube cookies found (yt-dlp exit {status}); is {browser} installed and logged in to YouTube?"
        );
        let part = dir.join("cookies.txt.part");
        create_private(&part)?.write_all(filtered.as_bytes())?;
        std::fs::rename(&part, &out)?;
        restrict(&out)
    })();
    let _ = std::fs::remove_file(&tmp);
    result
}

/// Use a cookies.txt the user exported (also the way in from a sandbox that cannot read browser
/// profiles): keep the YouTube/Google lines, write the private cookie file. Returns the count.
pub fn import_cookie_file(src: &Path) -> Result<usize> {
    import_cookie_file_to(src, &config::cookies_path())
}

fn import_cookie_file_to(src: &Path, out: &Path) -> Result<usize> {
    use std::io::Write;
    let text = std::fs::read_to_string(src).with_context(|| format!("read {}", src.display()))?;
    let filtered = filter_jar(&text);
    let n = filtered.lines().count() - 1;
    anyhow::ensure!(
        n > 0,
        "no YouTube/Google cookies in that file (Netscape cookies.txt format expected)"
    );
    let dir = out.parent().context("no data dir")?;
    std::fs::create_dir_all(dir)?;
    let part = dir.join("cookies.txt.part");
    create_private(&part)?.write_all(filtered.as_bytes())?;
    std::fs::rename(&part, out)?;
    restrict(out)?;
    Ok(n)
}

fn setup(browser: Option<String>) -> Result<i32> {
    let mut cfg = Config::load();
    let browser = browser.unwrap_or_else(|| cfg.browser.clone());
    export_cookies(&browser)?;
    cfg.browser = browser;
    cfg.save()?;
    println!("cookies exported to {}", config::cookies_path().display());
    check()
}

fn check() -> Result<i32> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let ok = rt.block_on(async { Client::new(Some(config::cookies_path()))?.signed_in().await })?;
    println!("signed in: {}", if ok { "yes" } else { "no" });
    Ok(if ok { 0 } else { 1 })
}

fn logout() -> Result<i32> {
    let p = config::cookies_path();
    match std::fs::remove_file(&p) {
        Ok(()) => println!("removed {}", p.display()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => println!("nothing to remove"),
        Err(e) => return Err(e.into()),
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::filter_jar;
    #[test]
    fn import_needs_google_cookies() {
        let dir = std::env::temp_dir().join(format!("cb-import-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("c.txt");
        std::fs::write(&f, ".bank.com\tTRUE\t/\tTRUE\t0\tc\t3\n").unwrap();
        assert!(super::import_cookie_file(&f).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_keeps_google_lines_private() {
        let dir = std::env::temp_dir().join(format!("cb-import-ok-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (f, out) = (dir.join("c.txt"), dir.join("out/cookies.txt"));
        std::fs::write(
            &f,
            ".youtube.com\tTRUE\t/\tTRUE\t0\ta\t1\n.bank.com\tTRUE\t/\tTRUE\t0\tc\t3\n",
        )
        .unwrap();
        assert_eq!(super::import_cookie_file_to(&f, &out).unwrap(), 1);
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.contains("\ta\t1") && !text.contains("bank"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(out.metadata().unwrap().permissions().mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn keeps_only_google_youtube() {
        let jar = "# Netscape HTTP Cookie File\n.youtube.com\tTRUE\t/\tTRUE\t0\ta\t1\n#HttpOnly_.google.com\tTRUE\t/\tTRUE\t0\tb\t2\n.bank.com\tTRUE\t/\tTRUE\t0\tc\t3\n.notgoogle.com\tTRUE\t/\tTRUE\t0\td\t4\n";
        let f = filter_jar(jar);
        assert!(f.contains("\ta\t1") && f.contains("\tb\t2"));
        assert!(!f.contains("bank") && !f.contains("notgoogle"));
    }
}
