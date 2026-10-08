use super::*;

/// Network + yt-dlp: measures AAC priming/padding. `cargo test real_track -- --ignored --nocapture`
#[test]
#[ignore]
fn real_track_priming_report() {
    let ytdlp = which_ytdlp();
    let r = super::super::resolver::Resolver::new(ytdlp, None);
    let opener = super::super::YtOpener { resolver: r };
    let o = opener.open("jNQXAC9IVRw").unwrap();
    let mut d = TrackDecoder::open(o.source, &o.ext).unwrap();
    let mut all = Vec::new();
    while let Some(b) = d.next_block().unwrap() {
        all.extend_from_slice(&b);
    }
    let frames = all.len() / 2;
    let lead = all
        .chunks(2)
        .take_while(|f| f[0].abs() < 1e-4 && f[1].abs() < 1e-4)
        .count();
    let trail = all
        .chunks(2)
        .rev()
        .take_while(|f| f[0].abs() < 1e-4 && f[1].abs() < 1e-4)
        .count();
    println!(
        "frames={frames} secs={:.4} reported_duration={:?} lead_silent={lead} trail_silent={trail}",
        frames as f64 / d.sample_rate as f64,
        d.duration_secs
    );
}

fn which_ytdlp() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(format!("{home}/.local/bin/yt-dlp"))
}
