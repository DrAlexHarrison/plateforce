//! One table, rendered two ways.
//!
//! The shorter rendering hides a column. It never drops a record: `provenance` is written in
//! both and `results.provenance_id` is present in both, so what differs is whether the table
//! view joins the chain into the display. A rendering that stopped recording would invert the
//! finding it exists to serve, which is that the undergraduate needs every choice recorded
//! more visibly than the graduate, not less.

use crate::engine::BatchResult;
use crate::relations::AggregateRow;

/// How much of the record the table view shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rendering {
    /// Every column, with the chain joined on.
    WithProvenance,
    /// The same rows and the same records, without the fingerprint column.
    WithoutProvenance,
}

/// A table a person reads, and the summary beneath it.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// One line per aggregated quantity, with its `n` beside it. Empty when nothing was bound.
    pub summary: Vec<String>,
    /// What the run walked, in the shape the conformance suite already uses.
    pub coverage: String,
}

impl BatchResult {
    /// One render function, one argument. There are not two front doors.
    pub fn render(&self, rendering: Rendering) -> Rendered {
        let hidden = ["provenance_id"];
        let full = crate::relations::ResultRow::header(&self.quantities);
        let keep: Vec<usize> = full
            .iter()
            .enumerate()
            .filter(|(_, name)| {
                rendering == Rendering::WithProvenance || !hidden.contains(&name.as_str())
            })
            .map(|(index, _)| index)
            .collect();

        let header = keep.iter().map(|index| full[*index].clone()).collect();
        let rows = self
            .results
            .iter()
            .map(|row| {
                let cells = row.cells(&self.quantities);
                keep.iter().map(|index| cells[*index].clone()).collect()
            })
            .collect();

        Rendered {
            header,
            rows,
            summary: self.aggregates.iter().map(summary_line).collect(),
            coverage: self.coverage.line(),
        }
    }
}

/// The mean row as the user reads it, with the count it was taken over beside it.
fn summary_line(row: &AggregateRow) -> String {
    let value = row
        .value
        .map(crate::relations::format_value)
        .unwrap_or_default();
    let dispersion = row
        .dispersion
        .map(|figure| format!(" sd {}", crate::relations::format_value(figure)))
        .unwrap_or_default();
    format!(
        "{} {} {}{}, n = {}",
        row.group_key, row.quantity, value, dispersion, row.n
    )
}
