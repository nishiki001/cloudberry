use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "cloudberry",
    version,
    about = "Unofficial client for YouTube Music, not affiliated with Google"
)]
pub struct Cli {
    /// Render a view with fake data to a PNG and exit.
    #[arg(long, value_name = "OUT.png")]
    pub screenshot: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "light")]
    pub theme: ThemeArg,
    #[arg(long, default_value = "main")]
    pub view: String,
    #[arg(long, default_value = "default")]
    pub fixture: String,
    /// Screenshot size, e.g. 860x540 (default 1200x760).
    #[arg(long, value_name = "WxH")]
    pub size: Option<String>,
    /// GUI smoke test: play this id on start.
    #[arg(long)]
    pub autoplay: Option<String>,
    /// GUI smoke test: quit after N seconds.
    #[arg(long)]
    pub quit_after: Option<u64>,
    /// GUI smoke test: open this playlist id in a tab on start.
    #[arg(long)]
    pub open_playlist: Option<String>,
    /// GUI smoke test: open this Discover section (home|new|charts|moods) on start.
    #[arg(long)]
    pub open_discover: Option<String>,
    /// GUI smoke test: drive the UI through search, Discover, tabs and "More like this".
    #[arg(long)]
    pub smoke: bool,
    /// Start in the mini player.
    #[arg(long)]
    pub mini: bool,
    #[command(subcommand)]
    pub command: Option<Cmd>,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum ThemeArg {
    Light,
    Dark,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Search the catalogue.
    Search {
        query: String,
        #[arg(long)]
        filter: Option<String>,
        /// Follow continuation pages until this many results (default: first page).
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    /// Play a video id or URL headlessly.
    Play {
        target: String,
        #[arg(long)]
        seconds: Option<u64>,
        /// After ~2 s of playback, seek to this position (seconds) and report where we land.
        #[arg(long)]
        seek: Option<u64>,
    },
    #[command(subcommand)]
    Auth(AuthCmd),
    /// List liked songs.
    Liked {
        #[arg(long)]
        json: bool,
    },
    /// Show a playlist (default: library playlists).
    Playlist {
        id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Radio for a video id.
    Radio {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Lyrics for a video id.
    Lyrics {
        id: String,
        #[arg(long)]
        json: bool,
    },
    /// Discover: home feed, new releases, charts, moods & genres.
    Discover {
        #[arg(long, default_value = "home")]
        section: String,
        /// Charts: ISO country code (ZZ = global).
        #[arg(long, default_value = "ZZ")]
        country: String,
        /// Moods: a category's params (from `--section moods`); related: a video id.
        #[arg(long)]
        params: Option<String>,
        /// Print the raw first response.
        #[arg(long)]
        raw: bool,
        #[arg(long)]
        json: bool,
    },
    /// An artist page by channel id (UC…) or name.
    Artist {
        target: String,
        #[arg(long)]
        json: bool,
    },
    /// Debugging: POST a JSON body to an InnerTube endpoint and print the raw response.
    #[command(hide = true)]
    Raw { endpoint: String, body: String },
    /// Check external dependencies.
    Doctor,
    /// Managed helper tools (yt-dlp, deno): `install`, `update` or `status` (default).
    Tools {
        #[arg(default_value = "status")]
        action: String,
    },
    /// Install the .desktop entry and icons for the current user (Linux).
    InstallDesktop {
        /// Install under this data dir instead of ~/.local/share.
        #[arg(long)]
        prefix: Option<PathBuf>,
    },
    /// Remove what `install-desktop` installed.
    UninstallDesktop {
        #[arg(long)]
        prefix: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum AuthCmd {
    Setup {
        #[arg(long)]
        browser: Option<String>,
    },
    /// Use a cookies.txt (Netscape format) exported from a browser; only YouTube/Google lines are kept.
    Import {
        file: PathBuf,
    },
    Check,
    Logout,
}
