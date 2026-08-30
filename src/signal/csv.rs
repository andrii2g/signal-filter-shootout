//! Strict headered scalar CSV ingestion.

use std::{fs::File, io::Read, path::Path};

use csv::{Reader, ReaderBuilder, StringRecord, Trim};
use thiserror::Error;

/// Column selection for a headered scalar CSV.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvReadOptions {
    pub value_column: String,
    pub time_column: String,
    pub reference_column: String,
}

impl Default for CsvReadOptions {
    fn default() -> Self {
        Self {
            value_column: "value".to_owned(),
            time_column: "timestamp".to_owned(),
            reference_column: "reference".to_owned(),
        }
    }
}

/// Parsed scalar measurements and optional aligned columns.
#[derive(Debug, Clone, PartialEq)]
pub struct SignalSeries {
    pub time: Option<Vec<f64>>,
    pub values: Vec<f64>,
    pub reference: Option<Vec<f64>>,
}

/// Actionable CSV schema and value errors.
#[derive(Debug, Error)]
pub enum CsvReadError {
    #[error("failed to read CSV: {0}")]
    Csv(#[from] csv::Error),

    #[error("CSV is missing required value column '{column}'")]
    MissingValueColumn { column: String },

    #[error("CSV contains no data rows")]
    Empty,

    #[error("CSV line {line}, column '{column}' is empty")]
    EmptyValue { line: u64, column: String },

    #[error("CSV line {line}, column '{column}' has invalid number '{value}'")]
    InvalidNumber {
        line: u64,
        column: String,
        value: String,
    },

    #[error("CSV line {line}, column '{column}' must be finite, got '{value}'")]
    NonFinite {
        line: u64,
        column: String,
        value: String,
    },
}

/// Read a headered CSV file from disk.
pub fn read_path(
    path: impl AsRef<Path>,
    options: &CsvReadOptions,
) -> Result<SignalSeries, CsvReadError> {
    let reader = ReaderBuilder::new()
        .trim(Trim::All)
        .from_reader(File::open(path).map_err(csv::Error::from)?);
    read_csv(reader, options)
}

/// Read a headered CSV from any byte stream.
pub fn read_reader(
    reader: impl Read,
    options: &CsvReadOptions,
) -> Result<SignalSeries, CsvReadError> {
    let reader = ReaderBuilder::new().trim(Trim::All).from_reader(reader);
    read_csv(reader, options)
}

fn read_csv(
    mut reader: Reader<impl Read>,
    options: &CsvReadOptions,
) -> Result<SignalSeries, CsvReadError> {
    let headers = reader.headers()?.clone();
    let value_index = column_index(&headers, &options.value_column).ok_or_else(|| {
        CsvReadError::MissingValueColumn {
            column: options.value_column.clone(),
        }
    })?;
    let time_index = column_index(&headers, &options.time_column);
    let reference_index = column_index(&headers, &options.reference_column);

    let mut values = Vec::new();
    let mut time = time_index.map(|_| Vec::new());
    let mut reference = reference_index.map(|_| Vec::new());

    for (record_index, record) in reader.records().enumerate() {
        let record = record?;
        let line = record
            .position()
            .map_or(record_index as u64 + 2, |position| position.line());
        values.push(parse_value(
            &record,
            value_index,
            line,
            &options.value_column,
        )?);
        if let (Some(index), Some(output)) = (time_index, time.as_mut()) {
            output.push(parse_value(&record, index, line, &options.time_column)?);
        }
        if let (Some(index), Some(output)) = (reference_index, reference.as_mut()) {
            output.push(parse_value(
                &record,
                index,
                line,
                &options.reference_column,
            )?);
        }
    }

    if values.is_empty() {
        return Err(CsvReadError::Empty);
    }

    Ok(SignalSeries {
        time,
        values,
        reference,
    })
}

fn column_index(headers: &StringRecord, name: &str) -> Option<usize> {
    headers.iter().position(|header| header == name)
}

fn parse_value(
    record: &StringRecord,
    index: usize,
    line: u64,
    column: &str,
) -> Result<f64, CsvReadError> {
    let text = record.get(index).unwrap_or_default();
    if text.is_empty() {
        return Err(CsvReadError::EmptyValue {
            line,
            column: column.to_owned(),
        });
    }
    let value = text
        .parse::<f64>()
        .map_err(|_| CsvReadError::InvalidNumber {
            line,
            column: column.to_owned(),
            value: text.to_owned(),
        })?;
    if !value.is_finite() {
        return Err(CsvReadError::NonFinite {
            line,
            column: column.to_owned(),
            value: text.to_owned(),
        });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{CsvReadError, CsvReadOptions, read_reader};

    #[test]
    fn reads_value_time_and_reference_columns() {
        let series = read_reader(
            "timestamp,value,reference\n0.0,2.0,1.5\n0.1,3.0,2.5\n".as_bytes(),
            &CsvReadOptions::default(),
        )
        .unwrap();

        assert_eq!(series.values, [2.0, 3.0]);
        assert_eq!(series.time, Some(vec![0.0, 0.1]));
        assert_eq!(series.reference, Some(vec![1.5, 2.5]));
    }

    #[test]
    fn reads_value_only_csv() {
        let series =
            read_reader("value\n1.0\n2.0\n".as_bytes(), &CsvReadOptions::default()).unwrap();

        assert_eq!(series.values, [1.0, 2.0]);
        assert_eq!(series.time, None);
        assert_eq!(series.reference, None);
    }

    #[test]
    fn supports_explicit_column_names() {
        let options = CsvReadOptions {
            value_column: "reading".to_owned(),
            time_column: "when".to_owned(),
            reference_column: "clean".to_owned(),
        };
        let series = read_reader("when,reading,clean\n4,5,6\n".as_bytes(), &options).unwrap();

        assert_eq!(series.values, [5.0]);
        assert_eq!(series.time, Some(vec![4.0]));
        assert_eq!(series.reference, Some(vec![6.0]));
    }

    #[test]
    fn rejects_missing_value_column() {
        assert!(matches!(
            read_reader(
                "timestamp,other\n0,1\n".as_bytes(),
                &CsvReadOptions::default()
            ),
            Err(CsvReadError::MissingValueColumn { .. })
        ));
    }

    #[test]
    fn malformed_number_includes_line_and_column() {
        let error = read_reader(
            "value\n1.0\nnot-a-number\n".as_bytes(),
            &CsvReadOptions::default(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            CsvReadError::InvalidNumber {
                line: 3,
                ref column,
                ..
            } if column == "value"
        ));
    }

    #[test]
    fn rejects_nan_infinity_empty_and_no_rows() {
        for text in ["value\nNaN\n", "value\ninf\n", "value\n-inf\n"] {
            assert!(matches!(
                read_reader(text.as_bytes(), &CsvReadOptions::default()),
                Err(CsvReadError::NonFinite { .. })
            ));
        }
        assert!(matches!(
            read_reader("value\n\n".as_bytes(), &CsvReadOptions::default()),
            Err(CsvReadError::EmptyValue { .. }) | Err(CsvReadError::Empty)
        ));
        assert!(matches!(
            read_reader("value\n".as_bytes(), &CsvReadOptions::default()),
            Err(CsvReadError::Empty)
        ));
    }
}
