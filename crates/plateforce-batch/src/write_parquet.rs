//! Tables in a container that has somewhere to put the record.
//!
//! Arrow schema metadata survives a Parquet round trip, so a file written here carries what
//! produced it and a lab that receives one file receives both. The key is `plateforce`, never
//! R's `r` block or pandas' `pandas` block, which are private to one language.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use arrow_array::{ArrayRef, Float64Array, RecordBatch, StringArray, UInt32Array};
use arrow_schema::{DataType, Field, Schema};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;

use crate::engine::BatchResult;
use crate::relations::RunRow;

/// The metadata key the fingerprint rides under.
pub const RUN_METADATA_KEY: &str = "plateforce";

#[derive(Debug, thiserror::Error)]
pub enum ParquetError {
    #[error("writing {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{relation}: {message}")]
    Arrow { relation: String, message: String },
    #[error("{path} carries no {RUN_METADATA_KEY} block, so nothing says what produced it")]
    RecordAbsent { path: String },
}

impl BatchResult {
    /// One file per relation under `directory`, each carrying the whole `run` block.
    pub fn write_parquet(&self, directory: &Path) -> Result<Vec<std::path::PathBuf>, ParquetError> {
        std::fs::create_dir_all(directory).map_err(|source| ParquetError::Io {
            path: directory.display().to_string(),
            source,
        })?;
        let record = serde_json::to_string(&self.run).unwrap_or_default();

        Ok(vec![
            self.write_relation(directory, "results", self.results_batch()?, &record)?,
            self.write_relation(directory, "provenance", self.provenance_batch()?, &record)?,
            self.write_relation(
                directory,
                "descriptions",
                self.descriptions_batch()?,
                &record,
            )?,
            self.write_relation(directory, "refusals", self.refusals_batch()?, &record)?,
            self.write_relation(directory, "signals", self.signals_batch()?, &record)?,
            self.write_relation(directory, "exclusions", self.exclusions_batch()?, &record)?,
        ])
    }

    fn write_relation(
        &self,
        directory: &Path,
        name: &str,
        batch: RecordBatch,
        record: &str,
    ) -> Result<std::path::PathBuf, ParquetError> {
        let path = directory.join(format!("{name}.parquet"));
        let file = std::fs::File::create(&path).map_err(|source| ParquetError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let metadata = HashMap::from([(RUN_METADATA_KEY.to_string(), record.to_string())]);
        let schema = Arc::new(batch.schema().as_ref().clone().with_metadata(metadata));
        let mut writer =
            ArrowWriter::try_new(file, schema, None).map_err(|error| ParquetError::Arrow {
                relation: name.to_string(),
                message: error.to_string(),
            })?;
        writer.write(&batch).map_err(|error| ParquetError::Arrow {
            relation: name.to_string(),
            message: error.to_string(),
        })?;
        writer.close().map_err(|error| ParquetError::Arrow {
            relation: name.to_string(),
            message: error.to_string(),
        })?;
        Ok(path)
    }

    fn results_batch(&self) -> Result<RecordBatch, ParquetError> {
        let mut fields = vec![
            Field::new("trial_id", DataType::Utf8, false),
            Field::new("source_path", DataType::Utf8, false),
            Field::new("provenance_id", DataType::Utf8, false),
            Field::new("refusal_code", DataType::Utf8, false),
        ];
        let mut columns: Vec<ArrayRef> = vec![
            text(self.results.iter().map(|row| row.trial_id.clone())),
            text(self.results.iter().map(|row| row.source_path.clone())),
            text(self.results.iter().map(|row| row.provenance_id.clone())),
            text(self.results.iter().map(|row| row.refusal_code.clone())),
        ];
        // A quantity stays a number rather than becoming text, so a reader opening this in R
        // or pandas gets the column it would have got from the analysis.
        for quantity in &self.quantities {
            fields.push(Field::new(quantity, DataType::Float64, true));
            columns.push(Arc::new(Float64Array::from(
                self.results
                    .iter()
                    .map(|row| row.values.get(quantity).copied().flatten())
                    .collect::<Vec<Option<f64>>>(),
            )) as ArrayRef);
        }
        batch("results", fields, columns)
    }

    fn provenance_batch(&self) -> Result<RecordBatch, ParquetError> {
        batch(
            "provenance",
            vec![
                Field::new("provenance_id", DataType::Utf8, false),
                Field::new("quantity", DataType::Utf8, false),
                Field::new("depth", DataType::UInt32, false),
                Field::new("method_id", DataType::Utf8, false),
                Field::new("parameter", DataType::Utf8, false),
                Field::new("value", DataType::Utf8, false),
                Field::new("source", DataType::Utf8, false),
            ],
            vec![
                text(self.provenance.iter().map(|row| row.provenance_id.clone())),
                text(self.provenance.iter().map(|row| row.quantity.clone())),
                counts(self.provenance.iter().map(|row| row.depth)),
                text(self.provenance.iter().map(|row| row.method_id.clone())),
                text(self.provenance.iter().map(|row| row.parameter.clone())),
                text(self.provenance.iter().map(|row| row.value.clone())),
                text(self.provenance.iter().map(|row| row.source.clone())),
            ],
        )
    }

    /// Beside `provenance` rather than folded into it: that relation collapses to one set of
    /// rows per distinct chain, and an account opens with the trial's own number.
    fn descriptions_batch(&self) -> Result<RecordBatch, ParquetError> {
        batch(
            "descriptions",
            vec![
                Field::new("trial_id", DataType::Utf8, false),
                Field::new("quantity", DataType::Utf8, false),
                Field::new("provenance_id", DataType::Utf8, false),
                Field::new("account", DataType::Utf8, false),
            ],
            vec![
                text(self.descriptions.iter().map(|row| row.trial_id.clone())),
                text(self.descriptions.iter().map(|row| row.quantity.clone())),
                text(
                    self.descriptions
                        .iter()
                        .map(|row| row.provenance_id.clone()),
                ),
                text(self.descriptions.iter().map(|row| row.account.clone())),
            ],
        )
    }

    fn refusals_batch(&self) -> Result<RecordBatch, ParquetError> {
        batch(
            "refusals",
            vec![
                Field::new("trial_id", DataType::Utf8, false),
                Field::new("ordinal", DataType::UInt32, false),
                Field::new("code", DataType::Utf8, false),
                Field::new("method_id", DataType::Utf8, false),
                Field::new("slot", DataType::Utf8, false),
                Field::new("parameter", DataType::Utf8, false),
                Field::new("value", DataType::Utf8, false),
                Field::new("detail", DataType::Utf8, false),
                Field::new("available", DataType::Utf8, false),
                Field::new("message", DataType::Utf8, false),
            ],
            vec![
                text(self.refusals.iter().map(|row| row.trial_id.clone())),
                counts(self.refusals.iter().map(|row| row.ordinal)),
                text(self.refusals.iter().map(|row| row.code.clone())),
                text(self.refusals.iter().map(|row| row.method_id.clone())),
                text(self.refusals.iter().map(|row| row.slot.clone())),
                text(self.refusals.iter().map(|row| row.parameter.clone())),
                text(self.refusals.iter().map(|row| row.value.clone())),
                text(self.refusals.iter().map(|row| row.detail.clone())),
                text(self.refusals.iter().map(|row| row.available.clone())),
                text(self.refusals.iter().map(|row| row.message.clone())),
            ],
        )
    }

    /// A signal's value is nullable and its threshold is not: a comparison that produced no
    /// number still ran against a stated one.
    fn signals_batch(&self) -> Result<RecordBatch, ParquetError> {
        batch(
            "signals",
            vec![
                Field::new("trial_id", DataType::Utf8, false),
                Field::new("ordinal", DataType::UInt32, false),
                Field::new("status", DataType::Utf8, false),
                Field::new("label", DataType::Utf8, false),
                Field::new("value", DataType::Float64, true),
                Field::new("unit", DataType::Utf8, false),
                Field::new("threshold", DataType::Float64, false),
                Field::new("qualifies", DataType::Utf8, false),
                Field::new("remedy_construct", DataType::Utf8, false),
                Field::new("remedy", DataType::Utf8, false),
            ],
            vec![
                text(self.signals.iter().map(|row| row.trial_id.clone())),
                counts(self.signals.iter().map(|row| row.ordinal)),
                text(self.signals.iter().map(|row| row.status.clone())),
                text(self.signals.iter().map(|row| row.label.clone())),
                Arc::new(Float64Array::from(
                    self.signals
                        .iter()
                        .map(|row| row.value)
                        .collect::<Vec<Option<f64>>>(),
                )) as ArrayRef,
                text(self.signals.iter().map(|row| row.unit.clone())),
                Arc::new(Float64Array::from(
                    self.signals
                        .iter()
                        .map(|row| row.threshold)
                        .collect::<Vec<f64>>(),
                )) as ArrayRef,
                text(self.signals.iter().map(|row| row.qualifies.clone())),
                text(self.signals.iter().map(|row| row.remedy_construct.clone())),
                text(self.signals.iter().map(|row| row.remedy.clone())),
            ],
        )
    }

    /// A gate's measured figure is nullable: a gate can match on a criterion it states in
    /// words without measuring a number for it.
    fn exclusions_batch(&self) -> Result<RecordBatch, ParquetError> {
        batch(
            "exclusions",
            vec![
                Field::new("trial_id", DataType::Utf8, false),
                Field::new("ordinal", DataType::UInt32, false),
                Field::new("method_id", DataType::Utf8, false),
                Field::new("outcome", DataType::Utf8, false),
                Field::new("parameter", DataType::Utf8, false),
                Field::new("value", DataType::Float64, true),
                Field::new("criterion", DataType::Utf8, false),
            ],
            vec![
                text(self.exclusions.iter().map(|row| row.trial_id.clone())),
                counts(0..self.exclusions.len()),
                text(self.exclusions.iter().map(|row| row.method_id.clone())),
                text(
                    self.exclusions
                        .iter()
                        .map(|row| if row.applied { "removed" } else { "reported" }.to_string()),
                ),
                text(
                    self.exclusions
                        .iter()
                        .map(|row| row.parameter.clone().unwrap_or_default()),
                ),
                Arc::new(Float64Array::from(
                    self.exclusions
                        .iter()
                        .map(|row| row.value)
                        .collect::<Vec<Option<f64>>>(),
                )) as ArrayRef,
                text(self.exclusions.iter().map(|row| row.criterion.clone())),
            ],
        )
    }
}

fn text(values: impl Iterator<Item = String>) -> ArrayRef {
    Arc::new(StringArray::from(values.collect::<Vec<String>>())) as ArrayRef
}

fn counts(values: impl Iterator<Item = usize>) -> ArrayRef {
    Arc::new(UInt32Array::from(
        values.map(|value| value as u32).collect::<Vec<u32>>(),
    )) as ArrayRef
}

fn batch(
    relation: &str,
    fields: Vec<Field>,
    columns: Vec<ArrayRef>,
) -> Result<RecordBatch, ParquetError> {
    RecordBatch::try_new(Arc::new(Schema::new(fields)), columns).map_err(|error| {
        ParquetError::Arrow {
            relation: relation.to_string(),
            message: error.to_string(),
        }
    })
}

/// The `run` block read back out of a file's own schema metadata.
pub fn read_run(path: &Path) -> Result<RunRow, ParquetError> {
    let file = std::fs::File::open(path).map_err(|source| ParquetError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let builder =
        ParquetRecordBatchReaderBuilder::try_new(file).map_err(|error| ParquetError::Arrow {
            relation: path.display().to_string(),
            message: error.to_string(),
        })?;
    let record = builder
        .schema()
        .metadata()
        .get(RUN_METADATA_KEY)
        .cloned()
        .ok_or_else(|| ParquetError::RecordAbsent {
            path: path.display().to_string(),
        })?;
    serde_json::from_str(&record).map_err(|error| ParquetError::Arrow {
        relation: path.display().to_string(),
        message: error.to_string(),
    })
}

/// The rows of one relation, read back as text and numbers in the order they were written.
pub fn read_relation(path: &Path) -> Result<Vec<RecordBatch>, ParquetError> {
    let file = std::fs::File::open(path).map_err(|source| ParquetError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)
        .and_then(|builder| builder.build())
        .map_err(|error| ParquetError::Arrow {
            relation: path.display().to_string(),
            message: error.to_string(),
        })?;
    reader
        .collect::<Result<Vec<RecordBatch>, _>>()
        .map_err(|error| ParquetError::Arrow {
            relation: path.display().to_string(),
            message: error.to_string(),
        })
}
