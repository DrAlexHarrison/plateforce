//! Tables on disk, and the record that has to go beside them.
//!
//! A bare CSV of numbers is the artefact that starts the whole problem, so the writer will
//! not produce one. The refusal lives here rather than in any one surface, so no surface can
//! route around it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::engine::BatchResult;
use crate::relations::{AggregateRow, ProvenanceRow, RefusalRow, ResultRow, RunRow, WarningRow};

/// One table a caller can ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    Results,
    Provenance,
    Refusals,
    Warnings,
    Aggregates,
    Run,
}

impl Relation {
    pub fn file_name(self) -> &'static str {
        match self {
            Relation::Results => "results.csv",
            Relation::Provenance => "provenance.csv",
            Relation::Refusals => "refusals.csv",
            Relation::Warnings => "warnings.csv",
            Relation::Aggregates => "aggregates.csv",
            Relation::Run => "run.json",
        }
    }
}

pub const EVERY_RELATION: &[Relation] = &[
    Relation::Run,
    Relation::Results,
    Relation::Provenance,
    Relation::Refusals,
    Relation::Warnings,
    Relation::Aggregates,
];

#[derive(Debug, thiserror::Error)]
pub enum WriteRefusal {
    #[error(
        "a table records numbers and carries no channel for what produced them, so {directory} takes run.json beside it"
    )]
    RecordNotRequested { directory: String },
    #[error("writing {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

impl BatchResult {
    /// Every relation, with the record. The four names §6 prints plus the two this run has a
    /// state for: warnings, and aggregates when a rule was bound.
    pub fn write_csv(&self, directory: &Path) -> Result<Vec<PathBuf>, WriteRefusal> {
        self.write_csv_selection(directory, EVERY_RELATION)
    }

    /// The record is written before any table, so a directory that cannot take it costs no
    /// half-written set.
    pub fn write_csv_selection(
        &self,
        directory: &Path,
        relations: &[Relation],
    ) -> Result<Vec<PathBuf>, WriteRefusal> {
        if !relations.contains(&Relation::Run) {
            return Err(WriteRefusal::RecordNotRequested {
                directory: directory.display().to_string(),
            });
        }
        std::fs::create_dir_all(directory).map_err(|source| WriteRefusal::Io {
            path: directory.display().to_string(),
            source,
        })?;

        let mut written = Vec::new();
        written.push(self.write_one(directory, Relation::Run)?);
        for relation in relations {
            if *relation == Relation::Run {
                continue;
            }
            if *relation == Relation::Aggregates && self.aggregates.is_empty() {
                continue;
            }
            written.push(self.write_one(directory, *relation)?);
        }
        Ok(written)
    }

    fn write_one(&self, directory: &Path, relation: Relation) -> Result<PathBuf, WriteRefusal> {
        let path = directory.join(relation.file_name());
        let body = match relation {
            Relation::Run => serde_json::to_string_pretty(&self.run).unwrap_or_default(),
            Relation::Results => table(
                ResultRow::header(&self.quantities),
                self.results.iter().map(|row| row.cells(&self.quantities)),
            ),
            Relation::Provenance => table(
                ProvenanceRow::header(),
                self.provenance.iter().map(ProvenanceRow::cells),
            ),
            Relation::Refusals => table(
                RefusalRow::header(),
                self.refusals.iter().map(RefusalRow::cells),
            ),
            Relation::Warnings => table(
                WarningRow::header(),
                self.warnings.iter().map(WarningRow::cells),
            ),
            Relation::Aggregates => table(
                AggregateRow::header(),
                self.aggregates.iter().map(AggregateRow::cells),
            ),
        };
        std::fs::write(&path, body).map_err(|source| WriteRefusal::Io {
            path: path.display().to_string(),
            source,
        })?;
        Ok(path)
    }
}

/// Written by hand rather than through a crate: six relations of scalar columns, and one
/// quoting rule for a path that contains a comma.
fn table(header: Vec<String>, rows: impl Iterator<Item = Vec<String>>) -> String {
    let mut text = String::new();
    text.push_str(&join(&header));
    text.push('\n');
    for row in rows {
        text.push_str(&join(&row));
        text.push('\n');
    }
    text
}

fn join(cells: &[String]) -> String {
    cells
        .iter()
        .map(|cell| quote(cell))
        .collect::<Vec<_>>()
        .join(",")
}

fn quote(cell: &str) -> String {
    if cell.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_string()
    }
}

/// The relations read back off disk, rebuilding the `run` row from the record beside the
/// table. That join is what the mandatory sidecar buys.
pub fn read_csv(directory: &Path) -> Result<ReadBack, WriteRefusal> {
    let read = |name: &str| -> Result<String, WriteRefusal> {
        let path = directory.join(name);
        std::fs::read_to_string(&path).map_err(|source| WriteRefusal::Io {
            path: path.display().to_string(),
            source,
        })
    };
    let run: RunRow =
        serde_json::from_str(&read("run.json")?).map_err(|source| WriteRefusal::Io {
            path: directory.join("run.json").display().to_string(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
        })?;

    let results_table = parse(&read("results.csv")?);
    let quantities: Vec<String> = results_table.header.iter().skip(4).cloned().collect();
    let results = results_table
        .rows
        .iter()
        .map(|cells| {
            let mut values = BTreeMap::new();
            for (index, quantity) in quantities.iter().enumerate() {
                let cell = cells.get(4 + index).cloned().unwrap_or_default();
                values.insert(
                    quantity.clone(),
                    if cell.is_empty() {
                        None
                    } else {
                        cell.parse::<f64>().ok()
                    },
                );
            }
            ResultRow {
                trial_id: cells[0].clone(),
                source_path: cells[1].clone(),
                provenance_id: cells[2].clone(),
                refusal_code: cells[3].clone(),
                values,
            }
        })
        .collect();

    let provenance = parse(&read("provenance.csv")?)
        .rows
        .iter()
        .map(|cells| ProvenanceRow {
            provenance_id: cells[0].clone(),
            quantity: cells[1].clone(),
            depth: cells[2].parse().unwrap_or_default(),
            method_id: cells[3].clone(),
            parameter: cells[4].clone(),
            value: cells[5].clone(),
            source: cells[6].clone(),
        })
        .collect();

    let refusals = parse(&read("refusals.csv")?)
        .rows
        .iter()
        .map(|cells| RefusalRow {
            trial_id: cells[0].clone(),
            ordinal: cells[1].parse().unwrap_or_default(),
            code: cells[2].clone(),
            method_id: cells[3].clone(),
            slot: cells[4].clone(),
            parameter: cells[5].clone(),
            value: cells[6].clone(),
            detail: cells[7].clone(),
            available: cells[8].clone(),
            message: cells[9].clone(),
        })
        .collect();

    Ok(ReadBack {
        run,
        quantities,
        results,
        provenance,
        refusals,
    })
}

/// What came back off disk, for the check that what went out comes back the same.
#[derive(Debug, Clone)]
pub struct ReadBack {
    pub run: RunRow,
    pub quantities: Vec<String>,
    pub results: Vec<ResultRow>,
    pub provenance: Vec<ProvenanceRow>,
    pub refusals: Vec<RefusalRow>,
}

struct Table {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

fn parse(text: &str) -> Table {
    let mut lines = split_records(text).into_iter();
    let header = lines.next().unwrap_or_default();
    Table {
        header,
        rows: lines.collect(),
    }
}

/// A quoted field may hold the record separator, so records are found by walking the text
/// rather than by splitting it on newlines.
fn split_records(text: &str) -> Vec<Vec<String>> {
    let mut records = Vec::new();
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut characters = text.chars().peekable();

    while let Some(character) = characters.next() {
        match (quoted, character) {
            (true, '"') if characters.peek() == Some(&'"') => {
                characters.next();
                cell.push('"');
            }
            (true, '"') => quoted = false,
            (true, other) => cell.push(other),
            (false, '"') => quoted = true,
            (false, ',') => cells.push(std::mem::take(&mut cell)),
            (false, '\n') => {
                cells.push(std::mem::take(&mut cell));
                records.push(std::mem::take(&mut cells));
            }
            (false, '\r') => {}
            (false, other) => cell.push(other),
        }
    }
    if !cell.is_empty() || !cells.is_empty() {
        cells.push(cell);
        records.push(cells);
    }
    records
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_carrying_the_separator_survives_the_round_trip() {
        let written = table(
            vec!["a".to_string(), "b".to_string()],
            std::iter::once(vec![
                "one,two".to_string(),
                "he said \"go\"\nthen left".to_string(),
            ]),
        );
        let parsed = parse(&written);
        assert_eq!(parsed.header, vec!["a", "b"]);
        assert_eq!(parsed.rows[0][0], "one,two");
        assert_eq!(parsed.rows[0][1], "he said \"go\"\nthen left");
    }
}
