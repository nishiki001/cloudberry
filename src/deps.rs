//! Locate external tools (mpv, yt-dlp, deno) and make them visible on PATH.
use std::path::{Path, PathBuf};

/// App id: Wayland app_id / X11 class, desktop file name, MPRIS bus name.
pub const APP_ID: &str = "io.github.nishiki001.Cloudberry";
pub const TOOLS: [&str; 3] = ["mpv", "yt-dlp", "deno"];

#[derive(Debug, Clone)]
pub struct Found {
    pub name: &'static str,
    pub path: Option<PathBuf>,
}

fn extra_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(h) = directories::BaseDirs::new() {
        v.push(h.home_dir().join(".local/bin"));
        v.push(h.home_dir().join(".deno/bin"));
    }
    v.push("/opt/homebrew/bin".into());
    v.push("/usr/local/bin".into());
    v
}

fn find_in(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    dirs.iter().map(|d| d.join(&exe)).find(|p| is_exec(p))
}

fn is_exec(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        p.metadata()
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

/// Search PATH plus the extra dirs; prepend any extra dir that holds a tool to PATH.
pub fn locate() -> Vec<Found> {
    static FOUND: std::sync::OnceLock<Vec<Found>> = std::sync::OnceLock::new();
    FOUND.get_or_init(locate_uncached).clone()
}

fn locate_uncached() -> Vec<Found> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    let mut prepend = Vec::new();
    // managed yt-dlp / deno win over system copies (unless the user or the build opted out)
    let managed = crate::tools::managed_enabled().then(crate::tools::managed_dir);
    if let Some(m) = managed.as_ref().filter(|m| m.is_dir()) {
        prepend.push(m.clone());
        dirs.insert(0, m.clone());
    }
    let extras = extra_dirs();
    let found: Vec<Found> = TOOLS
        .iter()
        .map(|&name| {
            let path = find_in(name, &dirs).or_else(|| {
                let p = find_in(name, &extras)?;
                let d = p.parent()?.to_path_buf();
                if !prepend.contains(&d) {
                    prepend.push(d);
                }
                Some(p)
            });
            Found { name, path }
        })
        .collect();
    // deno may sit in an extra dir even when yt-dlp is on PATH: expose them all.
    for d in extras {
        if d.is_dir()
            && !dirs.contains(&d)
            && !prepend.contains(&d)
            && TOOLS
                .iter()
                .any(|t| find_in(t, std::slice::from_ref(&d)).is_some())
        {
            prepend.push(d);
        }
    }
    if !prepend.is_empty() {
        dirs.retain(|d| !prepend.contains(d));
        prepend.append(&mut dirs);
        if let Ok(joined) = std::env::join_paths(prepend) {
            // SAFETY: called once at startup before any other thread exists.
            unsafe { std::env::set_var("PATH", joined) };
        }
    }
    found
}

#[allow(dead_code)] // used from M1
pub fn path_of(found: &[Found], name: &str) -> Option<PathBuf> {
    found
        .iter()
        .find(|f| f.name == name)
        .and_then(|f| f.path.clone())
}

pub fn install_help(missing: &[&str]) -> String {
    let (mpv, ytdlp, deno) = if cfg!(target_os = "macos") {
        (
            "brew install mpv",
            "brew install yt-dlp",
            "brew install deno",
        )
    } else {
        (
            "sudo dnf install mpv mpv-libs   (or: sudo apt install mpv libmpv2)",
            "pipx install yt-dlp   (or download from github.com/yt-dlp/yt-dlp)",
            "curl -fsSL https://deno.land/install.sh | sh",
        )
    };
    let mut s = String::from("Missing required tools:\n");
    for m in missing {
        let cmd = match *m {
            "mpv" => mpv,
            "yt-dlp" => ytdlp,
            _ => deno,
        };
        s.push_str(&format!("  {m}: {cmd}\n"));
    }
    s
}

/// Tools the chosen backend cannot run without: yt-dlp and deno always; mpv only for the
/// mpv backend (it is the optional fallback otherwise).
pub fn missing(found: &[Found], backend: &str) -> Vec<&'static str> {
    // without the `mpv` feature "mpv" means native
    let backend = if cfg!(feature = "mpv") {
        backend
    } else {
        "native"
    };
    found
        .iter()
        .filter(|f| f.path.is_none() && (f.name != "mpv" || backend == "mpv"))
        .map(|f| f.name)
        .collect()
}

/// `doctor` subcommand: print status, return process exit code.
pub fn doctor() -> i32 {
    let found = locate();
    for f in &found {
        match &f.path {
            Some(p) => println!("ok       {:7} {}", f.name, p.display()),
            None if f.name == "mpv" => println!(
                "optional {} (fallback backend; not needed with backend = \"native\")",
                f.name
            ),
            None => println!("MISSING  {}", f.name),
        }
    }
    let m = missing(&found, &crate::config::Config::load().backend);
    if m.is_empty() {
        0
    } else {
        eprint!("{}", install_help(&m));
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn find_in_empty() {
        assert!(find_in("definitely-not-a-tool", &[PathBuf::from("/nonexistent")]).is_none());
    }
    #[test]
    fn help_lists_missing() {
        let s = install_help(&["deno"]);
        assert!(s.contains("deno"));
        assert!(!s.contains("yt-dlp"));
    }
}
