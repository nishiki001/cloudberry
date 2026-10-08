//! LRC (synced lyrics) parsing.
use super::lyrics::Line;

/// Parse LRC text: `[mm:ss.xx] line` (several stamps per line allowed); tags like `[ar:..]` are skipped.
pub fn parse_lrc(text: &str) -> Vec<Line> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut stamps = Vec::new();
        while let Some(r) = rest.strip_prefix('[') {
            let Some(end) = r.find(']') else { break };
            match parse_stamp(&r[..end]) {
                Some(t) => stamps.push(t),
                None => {
                    stamps.clear();
                    rest = "";
                    break;
                }
            }
            rest = r[end + 1..].trim_start();
        }
        for t in stamps {
            out.push(Line {
                t,
                text: rest.trim().to_string(),
            });
        }
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    out
}

fn parse_stamp(s: &str) -> Option<f64> {
    let (m, rest) = s.split_once(':')?;
    let m: f64 = m.parse().ok()?;
    let sec: f64 = rest.parse().ok()?;
    (m >= 0.0 && (0.0..60.0).contains(&sec)).then_some(m * 60.0 + sec)
}
