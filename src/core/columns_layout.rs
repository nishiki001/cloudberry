//! Pixel layout of the playlist table and the column resize drag. Pure.
use super::{Col, ColumnConfig, HIDE_ORDER, Placed, SCROLL_W, STATE_W};

impl ColumnConfig {
    /// Layout for a table `total` px wide. The visible columns always fit exactly:
    /// Title / Artist / Album share what the fixed columns leave, in proportion to their saved
    /// widths (default 40 : 22 : 30), each at least its minimum. When even the minimums do not
    /// fit, columns are dropped automatically (Year, Album, Artist, Length, #, ♥).
    pub fn layout(&self, total: f32) -> Vec<Placed> {
        let avail = (total - SCROLL_W - STATE_W).max(0.0);
        let mut vis = self.visible();
        let need = |vis: &[Col]| -> f32 {
            vis.iter()
                .map(|c| {
                    if c.flexible() {
                        c.min_width()
                    } else {
                        self.width(*c)
                    }
                })
                .sum::<u32>() as f32
        };
        for drop in HIDE_ORDER {
            if need(&vis) <= avail {
                break;
            }
            vis.retain(|c| *c != drop);
        }
        let fixed: f32 = vis
            .iter()
            .filter(|c| !c.flexible())
            .map(|c| self.width(*c) as f32)
            .sum();
        let flex: Vec<Col> = vis.iter().copied().filter(|c| c.flexible()).collect();
        let shares = self.flex_widths(&flex, (avail - fixed).max(0.0));
        let mut x = STATE_W;
        vis.iter()
            .map(|&col| {
                let w = match flex.iter().position(|c| *c == col) {
                    Some(i) => shares[i],
                    None => self.width(col) as f32,
                };
                let p = Placed { col, x, w };
                x += w;
                p
            })
            .collect()
    }

    /// Split `room` px over the flexible columns by weight, whole pixels, minimums respected;
    /// the widths add up to exactly `room` (when the minimums fit).
    fn flex_widths(&self, flex: &[Col], room: f32) -> Vec<f32> {
        let mut out = vec![0.0f32; flex.len()];
        let mut open: Vec<usize> = (0..flex.len()).collect();
        let mut left = room.floor();
        // a column whose share is below its minimum is pinned at the minimum, the rest re-share
        loop {
            let weight: f32 = open.iter().map(|&i| self.width(flex[i]) as f32).sum();
            let pinned: Vec<usize> = open
                .iter()
                .copied()
                .filter(|&i| {
                    left * self.width(flex[i]) as f32 / weight < flex[i].min_width() as f32
                })
                .collect();
            if pinned.is_empty() || open.is_empty() {
                for (n, &i) in open.iter().enumerate() {
                    out[i] = if n + 1 == open.len() {
                        0.0 // the last one takes the remainder below
                    } else {
                        (left * self.width(flex[i]) as f32 / weight).floor()
                    };
                }
                if let Some(&last) = open.last() {
                    let used: f32 = open.iter().map(|&i| out[i]).sum();
                    out[last] = (left - used).max(flex[last].min_width() as f32);
                }
                return out;
            }
            for i in pinned {
                out[i] = flex[i].min_width() as f32;
                left -= out[i];
                open.retain(|&o| o != i);
            }
            if open.is_empty() {
                return out;
            }
        }
    }

    /// Apply a resize drag of `delta` px on the right edge of `col`, computed from the widths
    /// at the start of the drag (`base`) — a pure function of (base, col, delta), so repeating
    /// a drag event never changes the result. A fixed column changes its own width (the
    /// flexible ones give or take the difference). A flexible column trades width with its
    /// flexible neighbour (the next one, or the previous one for the last); the three weights
    /// are re-saved as the pixel widths shown, so the proportions hold at any window size.
    pub fn resize_from(&mut self, base: &ColumnConfig, col: Col, delta: f32, total: f32) {
        self.widths.clone_from(&base.widths);
        let layout = base.layout(total);
        let Some(i) = layout.iter().position(|p| p.col == col) else {
            return;
        };
        let delta = delta.round() as i64;
        if !col.flexible() {
            let flex_px: f32 = layout
                .iter()
                .filter(|p| p.col.flexible())
                .map(|p| p.w)
                .sum();
            let flex_min: u32 = layout
                .iter()
                .filter(|p| p.col.flexible())
                .map(|p| p.col.min_width())
                .sum();
            let max = layout[i].w as i64 + (flex_px as i64 - flex_min as i64).max(0);
            let w = (layout[i].w as i64 + delta)
                .clamp(col.min_width() as i64, max.max(col.min_width() as i64));
            self.set_width(col, w as u32);
            return;
        }
        let flex: Vec<usize> = (0..layout.len())
            .filter(|&k| layout[k].col.flexible())
            .collect();
        let at = flex.iter().position(|&k| k == i).unwrap_or(0);
        let (n, sign) = match flex.get(at + 1) {
            Some(&n) => (n, 1),
            None if at > 0 => (flex[at - 1], 1),
            None => return, // the only flexible column
        };
        let (a, b) = (layout[i].w as i64, layout[n].w as i64);
        let a2 = (a + sign * delta).clamp(
            col.min_width() as i64,
            (a + b - layout[n].col.min_width() as i64).max(col.min_width() as i64),
        );
        for &k in &flex {
            let w = if k == i {
                a2
            } else if k == n {
                a + b - a2
            } else {
                layout[k].w as i64
            };
            self.set_width(layout[k].col, w.max(1) as u32);
        }
    }
}
