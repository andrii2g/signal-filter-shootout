//! Shared-scale terminal frames with columns, rows, and automatic layout.

use super::sparkline::{render, shared_range};

/// Requested terminal arrangement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Auto,
    Columns,
    Rows,
}

/// Borrowed labeled series for rendering.
#[derive(Debug, Clone, Copy)]
pub struct TraceView<'a> {
    pub label: &'a str,
    pub values: &'a [f64],
}

/// Determine terminal width from COLUMNS, with a safe fallback and minimum.
pub fn available_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(80)
        .max(40)
}

/// Render all traces with a common vertical range.
pub fn render_frame(traces: &[TraceView<'_>], width: usize, layout: Layout) -> String {
    if traces.is_empty() {
        return String::new();
    }

    let Some(range) = shared_range(&traces.iter().map(|trace| trace.values).collect::<Vec<_>>())
    else {
        return String::new();
    };

    let columns_fit = width >= traces.len() * 24;
    match layout {
        Layout::Columns | Layout::Auto if columns_fit => render_columns(traces, width, range),
        Layout::Auto | Layout::Rows | Layout::Columns => render_rows(traces, width, range),
    }
}

fn render_rows(
    traces: &[TraceView<'_>],
    width: usize,
    range: super::sparkline::ValueRange,
) -> String {
    let label_width = traces
        .iter()
        .map(|trace| trace.label.chars().count())
        .max()
        .unwrap_or(0);
    let spark_width = width.saturating_sub(label_width + 3).max(1);

    traces
        .iter()
        .map(|trace| {
            format!(
                "{:<label_width$} │ {}",
                trace.label,
                render(trace.values, spark_width, range)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_columns(
    traces: &[TraceView<'_>],
    width: usize,
    range: super::sparkline::ValueRange,
) -> String {
    let panel_width = width / traces.len();
    let spark_width = panel_width.saturating_sub(1).max(1);
    let labels = traces
        .iter()
        .map(|trace| pad(trace.label, panel_width))
        .collect::<String>();
    let sparklines = traces
        .iter()
        .map(|trace| pad(&render(trace.values, spark_width, range), panel_width))
        .collect::<String>();

    format!("{}\n{}", labels.trim_end(), sparklines.trim_end())
}

fn pad(value: &str, width: usize) -> String {
    let length = value.chars().count();
    if length >= width {
        value.chars().take(width).collect()
    } else {
        format!("{value}{}", " ".repeat(width - length))
    }
}

#[cfg(test)]
mod tests {
    use super::{Layout, TraceView, render_frame};

    #[test]
    fn row_layout_contains_all_labels() {
        let frame = render_frame(
            &[
                TraceView {
                    label: "Raw",
                    values: &[0.0, 1.0],
                },
                TraceView {
                    label: "Kalman",
                    values: &[0.5, 0.75],
                },
            ],
            40,
            Layout::Rows,
        );

        assert!(frame.contains("Raw"));
        assert!(frame.contains("Kalman"));
        assert_eq!(frame.lines().count(), 2);
    }

    #[test]
    fn shared_range_makes_extremes_comparable() {
        let frame = render_frame(
            &[
                TraceView {
                    label: "Low",
                    values: &[0.0],
                },
                TraceView {
                    label: "High",
                    values: &[10.0],
                },
            ],
            40,
            Layout::Rows,
        );

        assert!(frame.lines().next().unwrap().contains('▁'));
        assert!(frame.lines().nth(1).unwrap().contains('█'));
    }

    #[test]
    fn auto_uses_columns_when_each_panel_has_room() {
        let frame = render_frame(
            &[
                TraceView {
                    label: "A",
                    values: &[0.0],
                },
                TraceView {
                    label: "B",
                    values: &[1.0],
                },
            ],
            100,
            Layout::Auto,
        );

        assert_eq!(frame.lines().count(), 2);
        assert!(frame.lines().next().unwrap().contains('A'));
        assert!(frame.lines().next().unwrap().contains('B'));
    }
}
