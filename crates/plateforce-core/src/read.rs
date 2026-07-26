//! Reading a delimited ASCII export into a trace.
//!
//! Which column carries vertical ground reaction force is a property of the export,
//! not of the file format, so the caller names it and the reader reports back what it
//! read rather than inferring anything.

use crate::signal::{Trial, TrialError};

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("line {line_number} has {columns_found} columns, so column index {column_index} does not exist")]
    ColumnMissing {
        line_number: usize,
        column_index: usize,
        columns_found: usize,
    },
    #[error("line {line_number} column {column_index} is not a number: {text:?}")]
    NotANumber {
        line_number: usize,
        column_index: usize,
        text: String,
    },
    #[error("no data rows found for column index {column_index}")]
    NoRows { column_index: usize },
    #[error("{0}")]
    Trace(#[from] TrialError),
    #[error("reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// What a read consumed. Emitted alongside the trace so a mis-set column
/// index shows up as a reported number rather than as a plausible looking result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnReadReport {
    pub column_index: usize,
    pub rows_read: usize,
    pub columns_per_row: usize,
    pub blank_lines_skipped: usize,
}

/// Read one numeric column out of delimited text.
///
/// Fields are trimmed before parsing because these exports carry the carriage return
/// of the machine that wrote them, and a trailing return turns the last column of
/// every row into a parse failure.
pub fn read_delimited_column(
    text: &str,
    delimiter: char,
    column_index: usize,
) -> Result<(Vec<f64>, ColumnReadReport), ReadError> {
    let mut values = Vec::new();
    let mut blank_lines_skipped = 0usize;
    let mut columns_per_row = 0usize;

    for (offset, line) in text.lines().enumerate() {
        let line_number = offset + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            blank_lines_skipped += 1;
            continue;
        }
        let fields: Vec<&str> = trimmed.split(delimiter).collect();
        columns_per_row = fields.len();
        let field = fields
            .get(column_index)
            .ok_or(ReadError::ColumnMissing {
                line_number,
                column_index,
                columns_found: fields.len(),
            })?
            .trim();
        let value = field.parse::<f64>().map_err(|_| ReadError::NotANumber {
            line_number,
            column_index,
            text: field.to_string(),
        })?;
        values.push(value);
    }

    if values.is_empty() {
        return Err(ReadError::NoRows { column_index });
    }
    let rows_read = values.len();
    Ok((
        values,
        ColumnReadReport {
            column_index,
            rows_read,
            columns_per_row,
            blank_lines_skipped,
        },
    ))
}

/// Read a trace from a file. The sample rate comes from the caller because these
/// exports do not carry one, and guessing it scales every velocity and height.
pub fn read_trial_from_path(
    path: impl AsRef<std::path::Path>,
    delimiter: char,
    column_index: usize,
    sample_rate_hz: f64,
) -> Result<(Trial, ColumnReadReport), ReadError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).map_err(|source| ReadError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let (values, report) = read_delimited_column(&text, delimiter, column_index)?;
    Ok((Trial::new(values, sample_rate_hz)?, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_carriage_return_at_the_end_of_a_row_does_not_break_the_last_column() {
        let text = "1.0\t2.0\t584.3485\r\n1.0\t2.0\t585.0\r\n";
        let (values, report) = read_delimited_column(text, '\t', 2).unwrap();
        assert_eq!(values, vec![584.3485, 585.0]);
        assert_eq!(report.rows_read, 2);
        assert_eq!(report.columns_per_row, 3);
    }

    #[test]
    fn a_missing_column_names_the_line_and_the_index_it_wanted() {
        let text = "1.0\t2.0\n";
        let error = read_delimited_column(text, '\t', 5).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("line 1"), "{message}");
        assert!(message.contains("column index 5"), "{message}");
    }

    #[test]
    fn a_non_numeric_field_names_the_text_it_could_not_parse() {
        let text = "1.0\t2.0\tnot-a-force\n";
        let error = read_delimited_column(text, '\t', 2).unwrap_err();
        assert!(error.to_string().contains("not-a-force"), "{error}");
    }
}
