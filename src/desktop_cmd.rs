//! `install-desktop` / `uninstall-desktop`: the .desktop entry and the icons for the current user.
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::deps::APP_ID as ID;

const DESKTOP: &str = include_str!("../packaging/io.github.nishiki001.Cloudberry.desktop");
const SIZES: &[(u32, &[u8])] = &[
    (
        16,
        include_bytes!("../assets/icons/app/png/cloudberry-16.png"),
    ),
    (
        22,
        include_bytes!("../assets/icons/app/png/cloudberry-22.png"),
    ),
    (
        24,
        include_bytes!("../assets/icons/app/png/cloudberry-24.png"),
    ),
    (
        32,
        include_bytes!("../assets/icons/app/png/cloudberry-32.png"),
    ),
    (
        48,
        include_bytes!("../assets/icons/app/png/cloudberry-48.png"),
    ),
    (
        64,
        include_bytes!("../assets/icons/app/png/cloudberry-64.png"),
    ),
    (
        128,
        include_bytes!("../assets/icons/app/png/cloudberry-128.png"),
    ),
    (
        256,
        include_bytes!("../assets/icons/app/png/cloudberry-256.png"),
    ),
    (
        512,
        include_bytes!("../assets/icons/app/png/cloudberry-512.png"),
    ),
];

/// The user's XDG data dir (`~/.local/share`), or `--prefix`.
fn data_dir(prefix: Option<&Path>) -> Result<PathBuf> {
    match prefix {
        Some(p) => Ok(p.to_path_buf()),
        None => directories::BaseDirs::new()
            .map(|d| d.data_dir().to_path_buf())
            .context("no home directory"),
    }
}

/// The desktop entry with `Exec=` pointing at this binary (the shipped file says `cloudberry`).
fn desktop_text(exe: &Path) -> String {
    // inside quotes ", `, $ and \ are backslash-escaped, and the string level doubles backslashes
    let quoted: String = exe
        .display()
        .to_string()
        .chars()
        .flat_map(|c| match c {
            '"' | '`' | '$' => vec!['\\', '\\', c],
            '\\' => vec!['\\'; 4],
            c => vec![c],
        })
        .collect();
    let exec = format!("Exec=\"{quoted}\"");
    DESKTOP
        .lines()
        .map(|l| {
            if l.starts_with("Exec=") {
                exec.as_str()
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn paths(data: &Path) -> (PathBuf, Vec<PathBuf>) {
    let icons = SIZES
        .iter()
        .map(|(s, _)| data.join(format!("icons/hicolor/{s}x{s}/apps/{ID}.png")))
        .collect();
    (data.join(format!("applications/{ID}.desktop")), icons)
}

/// Files of the first version (app id `cloudberry`): removed so upgraders get one launcher.
fn remove_legacy(data: &Path) {
    let _ = std::fs::remove_file(data.join("applications/cloudberry.desktop"));
    for (s, _) in SIZES {
        let _ =
            std::fs::remove_file(data.join(format!("icons/hicolor/{s}x{s}/apps/cloudberry.png")));
    }
}

pub fn install(prefix: Option<&Path>) -> Result<i32> {
    let data = data_dir(prefix)?;
    remove_legacy(&data);
    let exe = std::env::current_exe().context("cannot find this executable")?;
    let (desktop, icons) = paths(&data);
    for ((_, bytes), path) in SIZES.iter().zip(&icons) {
        std::fs::create_dir_all(path.parent().context("bad path")?)?;
        std::fs::write(path, bytes)?;
    }
    std::fs::create_dir_all(desktop.parent().context("bad path")?)?;
    std::fs::write(&desktop, desktop_text(&exe))?;
    println!(
        "installed {} and {} icons under {}",
        desktop.display(),
        icons.len(),
        data.display()
    );
    Ok(0)
}

pub fn uninstall(prefix: Option<&Path>) -> Result<i32> {
    let data = data_dir(prefix)?;
    remove_legacy(&data);
    let (desktop, icons) = paths(&data);
    let mut n = 0;
    for p in icons.iter().chain(std::iter::once(&desktop)) {
        if std::fs::remove_file(p).is_ok() {
            n += 1;
        }
    }
    println!("removed {n} files from {}", data.display());
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_then_uninstall_in_a_temp_prefix() {
        let dir = std::env::temp_dir().join(format!("cloudberry-desktop-{}", std::process::id()));
        install(Some(&dir)).unwrap();
        let desktop =
            std::fs::read_to_string(dir.join(format!("applications/{ID}.desktop"))).unwrap();
        assert!(
            desktop.contains(&format!("Icon={ID}"))
                && desktop.contains(&format!("StartupWMClass={ID}"))
        );
        assert!(desktop.contains("Exec=\"") && !desktop.contains("Exec=cloudberry\n"));
        let png = std::fs::read(dir.join(format!("icons/hicolor/48x48/apps/{ID}.png"))).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        uninstall(Some(&dir)).unwrap();
        assert!(!dir.join(format!("applications/{ID}.desktop")).exists());
        assert!(
            !dir.join(format!("icons/hicolor/48x48/apps/{ID}.png"))
                .exists()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
