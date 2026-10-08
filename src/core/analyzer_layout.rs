//! The single layout function of the analyzer: how many cells fit a length at a constant cell
//! size, and the equal margins that keep the result edge to edge. Used by every style (the
//! cell styles, and the band count that Wave and Curve sample their data with).

pub const MIN_BANDS: usize = 12;
pub const MAX_BANDS: usize = 512;

/// `count` cells of `cell` px with `gap` px between them, `margin` px free at each end.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Axis {
    pub count: usize,
    pub margin: u32,
}

/// floor((len + gap) / (cell + gap)) cells, clamped to `min..=max`; what is left over (less than
/// one cell + gap) is split into equal margins at both ends.
pub fn axis(cell: u32, gap: u32, len: u32, min: usize, max: usize) -> Axis {
    let pitch = cell + gap;
    let count = (((len + gap) / pitch) as usize).clamp(min, max);
    let used = (count as u32 * pitch).saturating_sub(gap);
    Axis {
        count,
        margin: len.saturating_sub(used) / 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_plus_margins_fill_the_length_edge_to_edge() {
        for (cell, gap) in [(6, 1), (4, 1), (4, 2), (5, 1)] {
            for len in (100..4000).step_by(13) {
                let a = axis(cell, gap, len, MIN_BANDS, MAX_BANDS);
                let used = a.count as u32 * (cell + gap) - gap;
                let right = len - used - a.margin;
                assert!(a.margin.abs_diff(right) <= 1, "{cell}+{gap} {len}: {a:?}");
                if a.count < MAX_BANDS {
                    assert!(a.margin < cell + gap, "margin below one cell: {a:?}");
                    assert_eq!(a.count as u32, (len + gap) / (cell + gap));
                }
            }
        }
    }

    #[test]
    fn counts_grow_with_the_length_and_are_clamped() {
        assert_eq!(axis(6, 1, 700, 12, 512).count, 100);
        assert_eq!(axis(6, 1, 1900, 12, 512).count, 271);
        assert_eq!(axis(6, 1, 10, 12, 512).count, 12);
        assert_eq!(axis(6, 1, 100_000, 12, 512).count, 512);
        // no 128 limit any more
        assert!(axis(4, 1, 1900, 12, 512).count > 128);
    }
}
