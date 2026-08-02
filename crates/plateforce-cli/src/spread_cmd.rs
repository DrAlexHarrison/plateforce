//! How far the choice of rule moves a number, on this trial.
//!
//! Not an advanced feature and never behind a flag. Five of six real course documents ask an
//! undergraduate to compute jump height two or three ways and explain the disagreement, so
//! this is their assignment rather than an expert affordance. It rides in `analyse`'s own
//! output for the headline quantity, and this command reports it for any other.

use std::fmt::Write as _;

use plateforce_analysis::spread::{run as sweep, Axis, SpreadRequest, SpreadResponse};
use plateforce_analysis::{bindings_for, AnalysisRequest};
use plateforce_core::Trial;

use crate::analyse::PATH;
use crate::decisions::slot_of;
use crate::render::{Renderer, Role};

/// The quantity `analyse` reports the spread for without being asked. Jump height is what the
/// audience came for and what the founding measurement is over.
pub const HEADLINE_QUANTITY: &str = "jump_height_from_takeoff_meters";

/// One axis per construct on the path, holding every rule this build can run for it. The
/// sweep varies the choice a user would otherwise make once and never revisit.
pub fn axes_over_every_rule() -> Vec<Axis> {
    PATH.iter()
        .map(|construct| {
            let slot = slot_of(construct);
            Axis {
                slot: slot.to_string(),
                parameter: None,
                values: Vec::new(),
                method_ids: bindings_for(slot)
                    .map(|binding| binding.id.to_string())
                    .collect(),
            }
        })
        .filter(|axis| axis.method_ids.len() > 1)
        .collect()
}

pub fn measure(
    trial: &Trial,
    request: &AnalysisRequest,
    quantity_key: &str,
) -> Result<SpreadResponse, String> {
    sweep(
        trial,
        &SpreadRequest {
            base: request.clone(),
            axes: axes_over_every_rule(),
            quantity_key: quantity_key.to_string(),
            maximum_combinations: 512,
        },
    )
}

/// The rules that produced the two ends of the spread, so the figure between them names
/// what it is a disagreement between.
fn extremes(response: &SpreadResponse) -> Vec<(String, String)> {
    let mut valued: Vec<(&str, f64, &[String])> = response
        .variants
        .iter()
        .filter_map(|variant| {
            variant
                .value
                .map(|value| (variant.label.as_str(), value, variant.method_ids.as_slice()))
        })
        .collect();
    if valued.len() < 2 {
        return Vec::new();
    }
    valued.sort_by(|left, right| left.1.total_cmp(&right.1));
    let lowest = valued.first().expect("two or more values");
    let highest = valued.last().expect("two or more values");
    [("lowest", lowest), ("highest", highest)]
        .into_iter()
        .map(|(end, (label, value, ids))| {
            let named = if ids.is_empty() {
                label.to_string()
            } else {
                ids.join(", ")
            };
            (format!("{end} {value:.4} {}", response.unit_symbol), named)
        })
        .collect()
}

/// The block `analyse` prints under the metrics. Every figure carries what it is taken over:
/// the percentage its median, the count its denominator.
pub fn describe(response: &SpreadResponse, renderer: &Renderer) -> String {
    let mut block = String::new();
    let _ = writeln!(
        block,
        "{}",
        renderer.paint(
            Role::Heading,
            &format!("Method spread, {}", response.quantity_key)
        )
    );

    match (
        response.minimum,
        response.maximum,
        response.median,
        response.spread_percent_of_median,
    ) {
        (Some(minimum), Some(maximum), Some(median), Some(percent)) => {
            let _ = writeln!(
                block,
                "  {minimum:.4} to {maximum:.4} {}, median {median:.4}, {percent:.1} percent of the median",
                response.unit_symbol
            );
        }
        // A sweep that produced no value is not a spread of zero, and printing one would be a
        // number no rule produced.
        _ => {
            let _ = writeln!(block, "  no combination produced a value");
        }
    }

    // A spread is two rules disagreeing, so the two rules are named. Left as a percentage
    // alone it reads as a property of the trial, and a single broken rule inside it reads as
    // a disagreement between sound ones.
    for (label, rules) in extremes(response) {
        let _ = writeln!(block, "  {label}");
        for line in renderer.wrap(&rules, 6) {
            let _ = writeln!(block, "{line}");
        }
    }

    let _ = write!(
        block,
        "  {} of {} combinations produced a value",
        response.succeeded, response.combinations_run
    );
    if response.capped {
        let _ = write!(
            block,
            ", of {} the rules on this path can make",
            response.combinations_requested
        );
    }
    // A variant that failed stays in the denominator and says why, because a spread taken
    // over the ones that worked is a spread over a set nobody chose.
    for variant in response.variants.iter().filter(|v| v.value.is_none()) {
        let reason = variant.failure_reason.as_deref().unwrap_or("no value");
        let _ = write!(block, "\n");
        let lines = renderer.wrap(&format!("{}: {reason}", variant.label), 4);
        let _ = write!(block, "{}", lines.join("\n"));
    }
    block
}
