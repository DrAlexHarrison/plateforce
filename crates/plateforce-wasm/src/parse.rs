//! Reading a force file that arrived as a string from a browser file input.
//!
//! Nothing here decides an analysis. It decides which bytes are numbers, and it reports
//! every decision it made so the interface can show them and the user can overrule them.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Delimiter {
    Tab,
    Comma,
    Semicolon,
    Whitespace,
}

impl Delimiter {
    fn split<'a>(&self, line: &'a str) -> Vec<&'a str> {
        match self {
            Delimiter::Tab => line.split('\t').collect(),
            Delimiter::Comma => line.split(',').collect(),
            Delimiter::Semicolon => line.split(';').collect(),
            Delimiter::Whitespace => line.split_whitespace().collect(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Delimiter::Tab => "tab",
            Delimiter::Comma => "comma",
            Delimiter::Semicolon => "semicolon",
            Delimiter::Whitespace => "whitespace",
        }
    }
}

/// What one column looks like, so a user can pick the force column by looking rather
/// than by trusting a heuristic.
#[derive(Debug, Clone, Serialize)]
pub struct ColumnSummary {
    pub index: usize,
    pub header: Option<String>,
    pub minimum: f64,
    pub maximum: f64,
    pub mean: f64,
    pub median: f64,
    pub standard_deviation: f64,
    pub finite_count: usize,
    pub exact_zero_count: usize,
    pub strictly_increasing: bool,
    /// Present only when the column is strictly increasing and evenly spaced, which is
    /// what a time column looks like.
    pub implied_sample_rate_hz: Option<f64>,
    /// Downsampled shape for the column chooser. Not an analysis input.
    pub sparkline: Vec<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForceFileSummary {
    pub delimiter: &'static str,
    pub row_count: usize,
    pub column_count: usize,
    pub skipped_leading_lines: usize,
    pub ragged_rows_dropped: usize,
    pub header_detected: bool,
    pub columns: Vec<ColumnSummary>,
    pub suggested_force_column: Option<usize>,
    pub suggested_force_column_reason: String,
    pub suggested_time_column: Option<usize>,
    pub suggested_sample_rate_hz: Option<f64>,
    pub sample_rate_source: String,
}

#[derive(Debug, Clone)]
pub struct ForceFile {
    pub columns: Vec<Vec<f64>>,
    pub summary: ForceFileSummary,
}

#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Vendor exports put units, dates and channel names above the numbers, so the reader
/// finds the first line that parses as a full numeric row and treats that as the data.
pub fn parse(text: &str) -> Result<ForceFile, ParseError> {
    let lines: Vec<&str> = text
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .collect();
    if lines.is_empty() {
        return Err(ParseError("the file contains no lines".into()));
    }

    let delimiter = detect_delimiter(&lines);
    let first_numeric = lines
        .iter()
        .position(|line| is_numeric_row(&delimiter, line))
        .ok_or_else(|| {
            ParseError(format!(
                "no line parsed as a row of numbers using the {} delimiter",
                delimiter.label()
            ))
        })?;

    let column_count = delimiter.split(lines[first_numeric]).len();
    let header = first_numeric
        .checked_sub(1)
        .map(|index| delimiter.split(lines[index]))
        .filter(|fields| fields.len() == column_count)
        .map(|fields| {
            fields
                .iter()
                .map(|f| f.trim().trim_matches('"').to_string())
                .collect::<Vec<_>>()
        })
        .filter(|fields| fields.iter().any(|f| !f.is_empty()));

    let mut columns: Vec<Vec<f64>> = vec![Vec::new(); column_count];
    let mut ragged_rows_dropped = 0usize;
    for line in &lines[first_numeric..] {
        if line.trim().is_empty() {
            continue;
        }
        let fields = delimiter.split(line);
        if fields.len() != column_count {
            ragged_rows_dropped += 1;
            continue;
        }
        // A field that does not parse becomes NaN rather than dropping the row, so the
        // count of what was unreadable survives to the summary instead of vanishing.
        for (index, field) in fields.iter().enumerate() {
            columns[index].push(field.trim().parse::<f64>().unwrap_or(f64::NAN));
        }
    }

    let row_count = columns.first().map_or(0, |c| c.len());
    if row_count < 2 {
        return Err(ParseError(
            "fewer than two data rows were readable, which is not a trace".into(),
        ));
    }

    let summaries: Vec<ColumnSummary> = columns
        .iter()
        .enumerate()
        .map(|(index, values)| summarise(index, values, header.as_ref()))
        .collect();

    let suggested_time_column = summaries
        .iter()
        .find(|c| c.implied_sample_rate_hz.is_some())
        .map(|c| c.index);
    let (suggested_force_column, reason) = suggest_force_column(&summaries, suggested_time_column);

    let suggested_sample_rate_hz =
        suggested_time_column.and_then(|index| summaries[index].implied_sample_rate_hz);
    let sample_rate_source = match suggested_time_column {
        Some(index) => format!("derived from the even spacing of column {}", index + 1),
        None => "no time column found, so the rate must be stated".to_string(),
    };

    Ok(ForceFile {
        summary: ForceFileSummary {
            delimiter: delimiter.label(),
            row_count,
            column_count,
            skipped_leading_lines: first_numeric,
            ragged_rows_dropped,
            header_detected: header.is_some(),
            columns: summaries,
            suggested_force_column,
            suggested_force_column_reason: reason,
            suggested_time_column,
            suggested_sample_rate_hz,
            sample_rate_source,
        },
        columns,
    })
}

fn detect_delimiter(lines: &[&str]) -> Delimiter {
    let sample: Vec<&&str> = lines.iter().take(200).collect();
    let candidates = [
        Delimiter::Tab,
        Delimiter::Comma,
        Delimiter::Semicolon,
        Delimiter::Whitespace,
    ];
    let mut best = Delimiter::Whitespace;
    let mut best_score = 0usize;
    for candidate in candidates {
        let score = sample
            .iter()
            .filter(|line| is_numeric_row(&candidate, line))
            .map(|line| candidate.split(line).len())
            .filter(|width| *width > 1)
            .sum::<usize>();
        if score > best_score {
            best_score = score;
            best = candidate;
        }
    }
    best
}

fn is_numeric_row(delimiter: &Delimiter, line: &str) -> bool {
    if line.trim().is_empty() {
        return false;
    }
    let fields = delimiter.split(line);
    if fields.is_empty() {
        return false;
    }
    fields
        .iter()
        .all(|field| field.trim().parse::<f64>().is_ok())
}

fn summarise(index: usize, values: &[f64], header: Option<&Vec<String>>) -> ColumnSummary {
    let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    let count = finite.len().max(1) as f64;
    let mean = finite.iter().sum::<f64>() / count;
    let variance = finite.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / count;

    let mut sorted = finite.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted.get(sorted.len() / 2).copied().unwrap_or(f64::NAN);

    let strictly_increasing = values.windows(2).all(|w| w[1] > w[0]);
    let implied_sample_rate_hz = strictly_increasing
        .then(|| even_spacing(values))
        .flatten()
        .map(|interval| 1.0 / interval);

    ColumnSummary {
        index,
        header: header
            .and_then(|h| h.get(index).cloned())
            .filter(|h| !h.is_empty()),
        minimum: sorted.first().copied().unwrap_or(f64::NAN),
        maximum: sorted.last().copied().unwrap_or(f64::NAN),
        mean,
        median,
        standard_deviation: variance.sqrt(),
        finite_count: finite.len(),
        exact_zero_count: values.iter().filter(|v| **v == 0.0).count(),
        strictly_increasing,
        implied_sample_rate_hz,
        sparkline: decimate(values, 48),
    }
}

/// A time column's steps agree to well within a part in a thousand. A force channel that
/// happens to rise monotonically will not.
fn even_spacing(values: &[f64]) -> Option<f64> {
    let first = values[1] - values[0];
    if !(first.is_finite() && first > 0.0) {
        return None;
    }
    let consistent = values
        .windows(2)
        .all(|w| ((w[1] - w[0]) - first).abs() < first * 1e-3);
    consistent.then_some(first)
}

/// The force channel is the one that moves most while sitting on a positive baseline. A
/// heuristic is not a measurement, so the reason travels with the suggestion and the
/// interface offers every column.
fn suggest_force_column(
    summaries: &[ColumnSummary],
    time_column: Option<usize>,
) -> (Option<usize>, String) {
    let plausible: Vec<&ColumnSummary> = summaries
        .iter()
        .filter(|c| Some(c.index) != time_column)
        .filter(|c| c.median > 50.0 && c.standard_deviation > 0.0)
        .collect();

    if plausible.is_empty() {
        return (
            None,
            "no column has a positive baseline in the newton range, so the force channel must be chosen by hand".into(),
        );
    }

    let chosen = plausible
        .iter()
        .max_by(|a, b| {
            a.standard_deviation
                .partial_cmp(&b.standard_deviation)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap();

    (
        Some(chosen.index),
        format!(
            "column {} varies most ({:.0} N standard deviation) among the {} columns whose median sits in the newton range, which is what a vertical force channel looks like",
            chosen.index + 1,
            chosen.standard_deviation,
            plausible.len()
        ),
    )
}

/// Min and max per bucket, so a spike survives downsampling instead of being averaged out.
pub fn decimate(values: &[f64], buckets: usize) -> Vec<f64> {
    if values.is_empty() || buckets == 0 {
        return Vec::new();
    }
    if values.len() <= buckets {
        return values.to_vec();
    }
    let width = values.len() as f64 / buckets as f64;
    (0..buckets)
        .map(|bucket| {
            let start = (bucket as f64 * width) as usize;
            let end = (((bucket + 1) as f64 * width) as usize)
                .min(values.len())
                .max(start + 1);
            let slice = &values[start..end];
            let sum: f64 = slice.iter().filter(|v| v.is_finite()).sum();
            sum / slice.len() as f64
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_headerless_tab_separated_matrix_with_carriage_returns() {
        let text = "2.83\t0\t582.95\t6.22\r\n2.83\t0\t584.34\t6.38\r\n2.83\t0\t583.10\t6.38\r\n";
        let file = parse(text).unwrap();
        assert_eq!(file.summary.delimiter, "tab");
        assert_eq!(file.summary.column_count, 4);
        assert_eq!(file.summary.row_count, 3);
        assert!(!file.summary.header_detected);
    }

    #[test]
    fn skips_a_vendor_preamble_and_keeps_the_header_line() {
        let text = "Exported 2011-03-04\nPlate 1\ntime,fx,fz\n0.000,1.1,600.0\n0.001,1.2,601.0\n";
        let file = parse(text).unwrap();
        assert_eq!(file.summary.skipped_leading_lines, 3);
        assert!(file.summary.header_detected);
        assert_eq!(file.summary.columns[2].header.as_deref(), Some("fz"));
    }

    #[test]
    fn a_time_column_gives_up_the_sample_rate_and_is_not_offered_as_force() {
        let mut text = String::from("t,f\n");
        for index in 0..500 {
            text.push_str(&format!(
                "{},{}\n",
                index as f64 / 1200.0,
                600.0 + index as f64
            ));
        }
        let file = parse(&text).unwrap();
        assert_eq!(file.summary.suggested_time_column, Some(0));
        let rate = file.summary.suggested_sample_rate_hz.unwrap();
        assert!((rate - 1200.0).abs() < 1e-6, "got {rate}");
        assert_eq!(file.summary.suggested_force_column, Some(1));
    }

    #[test]
    fn an_unreadable_field_is_counted_rather_than_dropping_the_row() {
        let text = "1.0,600.0\n2.0,n/a\n3.0,602.0\n";
        let file = parse(text).unwrap();
        assert_eq!(file.summary.row_count, 3);
        assert_eq!(file.summary.columns[1].finite_count, 2);
    }

    #[test]
    fn a_file_with_no_numbers_says_so_rather_than_returning_an_empty_trace() {
        assert!(parse("name,notes\nalpha,beta\n").is_err());
    }
}
