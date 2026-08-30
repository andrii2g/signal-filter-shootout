//! Unicode sparkline mapping and deterministic impulse-preserving downsampling.

/// Ordered sparkline glyphs from minimum to maximum.
pub const BLOCKS: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Shared vertical range for comparable traces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ValueRange {
    pub min: f64,
    pub max: f64,
}

/// Calculate one finite range across all supplied series.
pub fn shared_range(series: &[&[f64]]) -> Option<ValueRange> {
    let mut values = series.iter().flat_map(|values| values.iter().copied());
    let first = values.find(|value| value.is_finite())?;
    let mut min = first;
    let mut max = first;

    for value in values.filter(|value| value.is_finite()) {
        min = min.min(value);
        max = max.max(value);
    }

    Some(ValueRange { min, max })
}

/// Downsample to exactly the requested width when the input is longer.
///
/// Each bucket retains the sample with greatest absolute deviation from that
/// bucket's mean, with ties resolved toward the earliest sample.
pub fn downsample(values: &[f64], width: usize) -> Vec<f64> {
    if width == 0 {
        return Vec::new();
    }
    if values.len() <= width {
        return values.to_vec();
    }

    (0..width)
        .map(|bucket| {
            let start = bucket * values.len() / width;
            let end = (bucket + 1) * values.len() / width;
            let slice = &values[start..end];
            let mean = slice.iter().sum::<f64>() / slice.len() as f64;

            slice
                .iter()
                .copied()
                .enumerate()
                .max_by(|(left_index, left), (right_index, right)| {
                    (left - mean)
                        .abs()
                        .total_cmp(&(right - mean).abs())
                        .then_with(|| right_index.cmp(left_index))
                })
                .map(|(_, value)| value)
                .expect("non-empty bucket")
        })
        .collect()
}

/// Render values at the requested width using a caller-supplied shared range.
pub fn render(values: &[f64], width: usize, range: ValueRange) -> String {
    downsample(values, width)
        .into_iter()
        .map(|value| glyph(value, range))
        .collect()
}

fn glyph(value: f64, range: ValueRange) -> char {
    if range.min == range.max {
        return BLOCKS[(BLOCKS.len() - 1) / 2];
    }

    let normalized = ((value - range.min) / (range.max - range.min)).clamp(0.0, 1.0);
    let index = (normalized * (BLOCKS.len() - 1) as f64).round() as usize;
    BLOCKS[index]
}

#[cfg(test)]
mod tests {
    use super::{BLOCKS, ValueRange, downsample, render, shared_range};

    #[test]
    fn constant_series_uses_stable_middle_glyph() {
        assert_eq!(
            render(&[4.0; 4], 4, ValueRange { min: 4.0, max: 4.0 }),
            "▄▄▄▄"
        );
    }

    #[test]
    fn min_and_max_use_full_glyph_range() {
        assert_eq!(
            render(
                &[0.0, 10.0],
                2,
                ValueRange {
                    min: 0.0,
                    max: 10.0
                }
            ),
            format!("{}{}", BLOCKS[0], BLOCKS[7])
        );
    }

    #[test]
    fn shared_range_spans_all_traces() {
        assert_eq!(
            shared_range(&[&[1.0, 2.0], &[-4.0, 3.0]]),
            Some(ValueRange {
                min: -4.0,
                max: 3.0
            })
        );
    }

    #[test]
    fn downsampling_has_exact_requested_length() {
        assert_eq!(
            downsample(&(0..100).map(f64::from).collect::<Vec<_>>(), 13).len(),
            13
        );
    }

    #[test]
    fn downsampling_preserves_bucket_impulse() {
        let values = [0.0, 0.0, 10.0, 0.0, 0.0, 0.0];

        assert!(downsample(&values, 2).contains(&10.0));
    }
}
