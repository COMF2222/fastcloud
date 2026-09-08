//! The curve the equaliser's little screen draws.
//!
//! Winamp joins the ten band positions with a cubic spline rather than straight
//! lines, and paints one vertical run of pixels per column. This is the same
//! natural-spline solve Webamp uses (`EqualizerWindow/spline.js`), so the curve
//! matches theirs on the same slider positions.
//!
//! Kept apart from the renderer because it is arithmetic with no pixels in it,
//! and arithmetic is what tests can check.

/// The graph's height in skin pixels, which is [`crate::skin::layout::eq::GRAPH`]'s.
/// The width is that area's too; only the height is needed here, because the
/// curve spans the bands rather than the whole screen.
pub const HEIGHT: usize = 19;

/// How far apart the ten band points sit across the graph. Nine gaps of twelve
/// pixels spans 108 of the 113, and the two spare columns are the left padding.
pub const BAND_STEP: usize = 12;
pub const PADDING_LEFT: usize = 2;

/// One column of the curve: which rows to paint, top and height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    pub x: usize,
    pub top: usize,
    pub height: usize,
}

/// The curve for ten gains in dB, each `-range..=range`.
///
/// Returns one [`Column`] per pixel across the bands' span. A column is a *run*
/// rather than a single pixel because a steep stretch of the curve would
/// otherwise draw a dotted line: Winamp fills from the previous row to this
/// one, which is what makes the line continuous.
pub fn curve(gains_db: &[f32; 10], range: f32) -> Vec<Column> {
    let last = HEIGHT - 1;
    // Top of the graph is +range, bottom is -range.
    let to_row = |db: f32| {
        let fraction = (range - db.clamp(-range, range)) / (2.0 * range);
        (fraction * last as f32).round() as usize
    };
    let xs: Vec<f32> = (0..10).map(|i| (i * BAND_STEP) as f32).collect();
    let ys: Vec<f32> = gains_db.iter().map(|db| to_row(*db) as f32).collect();
    let all = spline(&xs, &ys);

    let mut columns = Vec::with_capacity(all.len());
    let mut previous = ys[0].round() as usize;
    for (x, y) in all.iter().enumerate() {
        let row = (y.round().max(0.0) as usize).min(last);
        let top = row.min(previous);
        let height = 1 + previous.abs_diff(row);
        columns.push(Column {
            x: PADDING_LEFT + x,
            top,
            height,
        });
        previous = row;
    }
    columns
}

/// Where the preamp's line sits, as a row. Winamp draws a one-pixel line the
/// width of the graph at the preamp's height.
pub fn preamp_row(preamp_db: f32, range: f32) -> usize {
    let last = HEIGHT - 1;
    let fraction = (range - preamp_db.clamp(-range, range)) / (2.0 * range);
    ((fraction * last as f32).round() as usize).min(last)
}

/// A natural cubic spline through `(xs, ys)`, sampled at every whole `x`.
fn spline(xs: &[f32], ys: &[f32]) -> Vec<f32> {
    let ks = natural_ks(xs, ys);
    let max_x = *xs.last().unwrap_or(&0.0) as usize;
    let mut out = Vec::with_capacity(max_x + 1);
    let mut i = 1usize;
    for x in 0..=max_x {
        let x = x as f32;
        while i + 1 < xs.len() && xs[i] < x {
            i += 1;
        }
        let span = xs[i] - xs[i - 1];
        // Guard the degenerate case: two points at the same x would divide by
        // zero and paint a column of NaN.
        if span.abs() < f32::EPSILON {
            out.push(ys[i]);
            continue;
        }
        let t = (x - xs[i - 1]) / span;
        let a = ks[i - 1] * span - (ys[i] - ys[i - 1]);
        let b = -ks[i] * span + (ys[i] - ys[i - 1]);
        let q = (1.0 - t) * ys[i - 1] + t * ys[i] + t * (1.0 - t) * (a * (1.0 - t) + b * t);
        out.push(q);
    }
    out
}

/// The spline's slopes, from the tridiagonal system a natural spline gives.
fn natural_ks(xs: &[f32], ys: &[f32]) -> Vec<f32> {
    let n = xs.len() - 1;
    // One row per point, one column per point plus the right-hand side.
    let mut m = vec![vec![0.0f32; n + 2]; n + 1];
    for i in 1..n {
        let left = xs[i] - xs[i - 1];
        let right = xs[i + 1] - xs[i];
        m[i][i - 1] = 1.0 / left;
        m[i][i] = 2.0 * (1.0 / left + 1.0 / right);
        m[i][i + 1] = 1.0 / right;
        m[i][n + 1] =
            3.0 * ((ys[i] - ys[i - 1]) / (left * left) + (ys[i + 1] - ys[i]) / (right * right));
    }
    let first = xs[1] - xs[0];
    m[0][0] = 2.0 / first;
    m[0][1] = 1.0 / first;
    m[0][n + 1] = 3.0 * (ys[1] - ys[0]) / (first * first);

    let last = xs[n] - xs[n - 1];
    m[n][n - 1] = 1.0 / last;
    m[n][n] = 2.0 / last;
    m[n][n + 1] = 3.0 * (ys[n] - ys[n - 1]) / (last * last);

    solve(m)
}

/// Gaussian elimination with partial pivoting.
fn solve(mut m: Vec<Vec<f32>>) -> Vec<f32> {
    let rows = m.len();
    for k in 0..rows {
        // Pivot on the largest *magnitude* in this column. Comparing signed
        // values would prefer a tiny positive to a large negative; this system's
        // coefficients are all positive so it made no difference, but that is a
        // property of the spline, not of the solver.
        let pivot = (k..rows)
            .max_by(|a, b| m[*a][k].abs().total_cmp(&m[*b][k].abs()))
            .unwrap_or(k);
        m.swap(k, pivot);
        let diagonal = m[k][k];
        if diagonal.abs() < f32::EPSILON {
            continue;
        }
        // The pivot row is read while the rows under it are written, so the two
        // halves are split rather than indexed.
        let (done, rest) = m.split_at_mut(k + 1);
        let pivot_row = &done[k];
        for row in rest {
            let factor = row[k] / diagonal;
            for (cell, above) in row.iter_mut().zip(pivot_row).skip(k + 1) {
                *cell -= above * factor;
            }
            row[k] = 0.0;
        }
    }
    let mut ks = vec![0.0f32; rows];
    for i in (0..rows).rev() {
        let diagonal = m[i][i];
        let value = if diagonal.abs() < f32::EPSILON {
            0.0
        } else {
            m[i][rows] / diagonal
        };
        ks[i] = value;
        for row in m.iter_mut().take(i) {
            row[rows] -= row[i] * value;
            row[i] = 0.0;
        }
    }
    ks
}

#[cfg(test)]
mod tests {
    use super::*;

    const RANGE: f32 = crate::audio::dsp::EQ_RANGE_DB;

    /// The curve spans the bands and stays on the screen, whatever the gains.
    #[test]
    fn the_curve_stays_inside_the_graph() {
        for gains in [
            [0.0; 10],
            [RANGE; 10],
            [-RANGE; 10],
            [
                RANGE, -RANGE, RANGE, -RANGE, RANGE, -RANGE, RANGE, -RANGE, RANGE, -RANGE,
            ],
            // Out of range, which the DSP clamps too.
            [100.0; 10],
        ] {
            let columns = curve(&gains, RANGE);
            assert_eq!(
                columns.len(),
                9 * BAND_STEP + 1,
                "the curve does not span the bands"
            );
            for column in &columns {
                assert!(
                    column.x < crate::skin::layout::eq::GRAPH.width as usize,
                    "column {} leaves the graph",
                    column.x
                );
                assert!(column.top < HEIGHT, "a column starts off the graph");
                assert!(
                    column.top + column.height <= HEIGHT,
                    "a column runs off the bottom: {column:?}"
                );
                assert!(column.height >= 1, "an empty column draws nothing");
            }
            // Every column is one pixel right of the last: no gaps, no repeats.
            for pair in columns.windows(2) {
                assert_eq!(pair[1].x, pair[0].x + 1);
            }
        }
    }

    /// Flat sliders draw a flat line down the middle, and the ends of the range
    /// draw at the top and the bottom.
    #[test]
    fn flat_sliders_draw_a_flat_middle_line() {
        let middle = (HEIGHT - 1) / 2;
        for column in curve(&[0.0; 10], RANGE) {
            assert_eq!(column.top, middle, "the flat line moved: {column:?}");
            assert_eq!(column.height, 1, "a flat line is one pixel tall");
        }
        // Fully up is the top row, fully down is the last.
        assert_eq!(curve(&[RANGE; 10], RANGE)[0].top, 0);
        assert_eq!(curve(&[-RANGE; 10], RANGE)[0].top, HEIGHT - 1);
    }

    /// The curve passes through the band positions it was given: a spline that
    /// misses its own control points is drawing the wrong thing.
    #[test]
    fn the_curve_hits_its_band_points() {
        let mut gains = [0.0; 10];
        gains[0] = RANGE;
        gains[9] = -RANGE;
        let columns = curve(&gains, RANGE);
        let at = |x: usize| {
            columns
                .iter()
                .find(|c| c.x == PADDING_LEFT + x)
                .expect("a column at that x")
        };
        // First band at the top, last at the bottom.
        assert_eq!(at(0).top, 0);
        assert_eq!(
            at(9 * BAND_STEP).top + at(9 * BAND_STEP).height - 1,
            HEIGHT - 1
        );
        // A steep stretch fills rather than dotting: somewhere between the two
        // ends there is a column taller than one pixel.
        assert!(
            columns.iter().any(|c| c.height > 1),
            "the line is dotted where it is steep"
        );
    }

    /// A single raised band bulges upward around itself and leaves the rest of
    /// the line where it was.
    #[test]
    fn one_raised_band_bulges_locally() {
        let mut gains = [0.0; 10];
        gains[5] = RANGE;
        let columns = curve(&gains, RANGE);
        let row_at = |x: usize| {
            columns
                .iter()
                .find(|c| c.x == PADDING_LEFT + x)
                .map(|c| c.top)
                .expect("a column")
        };
        let middle = (HEIGHT - 1) / 2;
        assert_eq!(
            row_at(5 * BAND_STEP),
            0,
            "the raised band is not at the top"
        );
        assert!(row_at(0) >= middle - 1, "the first band moved with it");
        assert!(row_at(9 * BAND_STEP) >= middle - 1, "the last band moved");
    }

    /// The preamp's line runs from the top of the graph to the bottom over its
    /// range, and stays on the screen outside it.
    #[test]
    fn the_preamp_line_spans_the_graph() {
        assert_eq!(preamp_row(RANGE, RANGE), 0);
        assert_eq!(preamp_row(0.0, RANGE), (HEIGHT - 1) / 2);
        assert_eq!(preamp_row(-RANGE, RANGE), HEIGHT - 1);
        assert_eq!(preamp_row(100.0, RANGE), 0, "clamped");
        assert_eq!(preamp_row(-100.0, RANGE), HEIGHT - 1, "clamped");
    }

    /// Ten points twelve pixels apart fit the graph with room for the padding.
    #[test]
    fn the_bands_fit_the_graph() {
        assert_eq!(PADDING_LEFT + 9 * BAND_STEP + 1, 111);
        assert!(PADDING_LEFT + 9 * BAND_STEP < crate::skin::layout::eq::GRAPH.width as usize);
        assert_eq!(HEIGHT, crate::skin::layout::eq::GRAPH.height as usize);
    }
}
