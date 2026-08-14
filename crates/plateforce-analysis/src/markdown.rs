//! A result as Markdown, for pasting where a reader is already talking.
//!
//! The destination is a chat box, so this is plain Markdown: a table and fenced blocks, no
//! HTML. What decides the content is not the format.
//!
//! A block that pastes numbers without the methods that produced them is this project's
//! founding defect with a clipboard attached. The reader hands a model a jump height and the
//! model has no way to know which of ten published rules produced it, which is the state of
//! every competing tool. So every block carries the method ids, the values each rule was bound
//! to and where each of those came from, the registry revision and its digest, and whether the
//! acquisition block is complete, beside the numbers. A block that would not let a second lab
//! reproduce the number is not the block to ship.
//!
//! One home, called by the terminal's `--format markdown` and by the browser's copy buttons, so
//! a reader piping a result into a model from a script and a reader pressing a button get the
//! same bytes.

use std::fmt::Write;

use crate::document::ResultDocument;

/// The whole result, table and provenance.
pub fn result(document: &ResultDocument) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# plateforce result: {}", escape(&document.trial.name));
    out.push('\n');
    numbers(&mut out, document, None);
    windows(&mut out, document);
    said(&mut out, document);
    provenance(&mut out, document);
    out
}

/// The numbers taken over one interval, with the same provenance behind them.
///
/// `over` names the rule whose window the caller is asking about, and only the quantities whose
/// chain reaches it are written. The rest are the trial's numbers rather than that window's, and
/// a block that carried them under a window's heading would say the window produced them.
pub fn window(document: &ResultDocument, over: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# plateforce window: {}", escape(&document.trial.name));
    out.push('\n');
    numbers(&mut out, document, Some(over));
    landmarks_in_window(&mut out, document);
    windows(&mut out, document);
    said(&mut out, document);
    provenance(&mut out, document);
    out
}

/// One row per quantity, and the rules behind it in the row rather than in a footnote: a
/// reader who copies three of these rows into a message keeps the attribution with the number.
///
/// Every rule the record names, not only the arithmetic one. A quantity that is a landmark
/// rule's own answer carries no `computed_by`, which is four of the eleven a plain run reports
/// and exactly the four whose rule choice moves the answer furthest. The column used to fall
/// back to a phrase naming this software as the author of those, which is the misattribution
/// the registry exists to prevent, wearing the costume of a table cell. Which rule roots a
/// chain is `chain.rs`'s answer and is not re-derived here: a second rooting rule that
/// disagreed with the first would be two records of one figure.
fn numbers(out: &mut String, document: &ResultDocument, over: Option<&str>) {
    let rows: Vec<&crate::response::Metric> = document
        .metrics
        .iter()
        .filter(|metric| match over {
            None => true,
            Some(rule) => {
                metric.computed_by.as_deref() != Some(rule)
                    && metric.contributing_method_ids.iter().any(|id| id == rule)
            }
        })
        .collect();
    if rows.is_empty() {
        let _ = writeln!(
            out,
            "No quantity on this path was taken over that interval.\n"
        );
        return;
    }
    let _ = writeln!(out, "| Quantity | Value | Unit | Rules |");
    let _ = writeln!(out, "|---|---|---|---|");
    for metric in rows {
        let value = match metric.value {
            Some(number) => format!("{number:.4}"),
            None => "no value on this trial".to_string(),
        };
        let behind = rules(document, &metric.key);
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            escape(&metric.label),
            value,
            escape(&metric.unit),
            if behind.is_empty() {
                "no rule recorded".to_string()
            } else {
                rule_list(&behind)
            },
        );
    }
    out.push('\n');
}

struct Landmark<'a> {
    label: &'static str,
    seconds: f64,
    rules: Vec<&'a str>,
}

fn metric<'a>(document: &'a ResultDocument, key: &str) -> Option<&'a crate::response::Metric> {
    document.metrics.iter().find(|metric| metric.key == key)
}

fn value(document: &ResultDocument, key: &str) -> Option<f64> {
    metric(document, key).and_then(|metric| metric.value)
}

fn rules<'a>(document: &'a ResultDocument, key: &str) -> Vec<&'a str> {
    let Some(metric) = metric(document, key) else {
        return Vec::new();
    };
    let mut found: Vec<&str> = Vec::new();
    if let Some(computed_by) = metric.computed_by.as_deref() {
        found.push(computed_by);
    }
    for method_id in &metric.contributing_method_ids {
        if !found.contains(&method_id.as_str()) {
            found.push(method_id);
        }
    }
    found
}

fn rule_list(rules: &[&str]) -> String {
    rules
        .iter()
        .map(|method_id| format!("`{}`", escape(method_id)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn landmark<'a>(
    document: &'a ResultDocument,
    label: &'static str,
    value_key: &str,
) -> Option<Landmark<'a>> {
    let seconds = value(document, value_key)?;
    let rules = rules(document, value_key);
    (!rules.is_empty()).then_some(Landmark {
        label,
        seconds,
        rules,
    })
}

fn landing<'a>(document: &'a ResultDocument) -> Option<Landmark<'a>> {
    let seconds =
        value(document, "takeoff_time_seconds")? + value(document, "flight_time_seconds")?;
    let rules = rules(document, "flight_time_seconds");
    (!rules.is_empty()).then_some(Landmark {
        label: "Landing",
        seconds,
        rules,
    })
}

/// The visible events inside a copied interval, and the phase around an interval that contains
/// none. The values come from the result document and every one stays attached to the rule chain
/// already recorded behind it. Nothing here reads the trace or computes a second landmark.
fn landmarks_in_window(out: &mut String, document: &ResultDocument) {
    let (Some(start), Some(end)) = (
        value(document, "analysis_window_start_seconds"),
        value(document, "analysis_window_end_seconds"),
    ) else {
        return;
    };

    let onset = landmark(document, "Movement onset", "onset_time_seconds");
    let takeoff = landmark(document, "Takeoff", "takeoff_time_seconds");
    let landing = landing(document);
    let landmarks = [onset.as_ref(), takeoff.as_ref(), landing.as_ref()];
    let inside: Vec<&Landmark<'_>> = landmarks
        .iter()
        .flatten()
        .copied()
        .filter(|event| event.seconds >= start && event.seconds <= end)
        .collect();

    let _ = writeln!(out, "## Landmarks in this window\n");
    if inside.is_empty() {
        let _ = writeln!(
            out,
            "No movement onset, takeoff, or landing falls inside this window.\n"
        );
    } else {
        for event in inside {
            let _ = writeln!(
                out,
                "- {} at {:.4} s, under {}",
                event.label,
                event.seconds,
                rule_list(&event.rules),
            );
        }
        out.push('\n');
    }

    let (Some(takeoff), Some(landing)) = (takeoff.as_ref(), landing.as_ref()) else {
        return;
    };
    if start > takeoff.seconds && end < landing.seconds {
        let _ = writeln!(out, "## Where this window sits\n");
        let _ = writeln!(
            out,
            "This window is inside flight, between takeoff at {:.4} s under {} and landing at {:.4} s under {}.\n",
            takeoff.seconds,
            rule_list(&takeoff.rules),
            landing.seconds,
            rule_list(&landing.rules),
        );
    }
}

/// The intervals this run's own rules settled, named so a reader can ask for one of them again.
fn windows(out: &mut String, document: &ResultDocument) {
    if document.regions.is_empty() {
        return;
    }
    let _ = writeln!(out, "## Intervals this run placed\n");
    for region in &document.regions {
        let _ = writeln!(
            out,
            "- `{}`, {:.4} to {:.4} s, placed by {}",
            region.phase,
            region.start_seconds,
            region.end_seconds,
            region.placed_by.join(", "),
        );
    }
    out.push('\n');
}

/// What the rules said beyond their numbers. A rule that declined is the answer on that trial,
/// so a block that carried the numbers and dropped the refusals would report a partial run as a
/// whole one.
fn said(out: &mut String, document: &ResultDocument) {
    if document.refusals.is_empty() && document.warnings.is_empty() {
        return;
    }
    let _ = writeln!(out, "## What the rules said\n");
    for refusal in &document.refusals {
        let _ = writeln!(out, "- Declined: {}", escape(refusal.message()));
    }
    for warning in &document.warnings {
        let _ = writeln!(out, "- {}", escape(warning));
    }
    out.push('\n');
}

/// Everything a second lab needs to run this again, in one fenced block so a paste keeps its
/// shape wherever it lands.
fn provenance(out: &mut String, document: &ResultDocument) {
    let _ = writeln!(out, "## Methods\n");
    let _ = writeln!(out, "```");
    let _ = writeln!(out, "plateforce {}", document.plateforce_version);
    let _ = writeln!(
        out,
        "registry revision {}",
        document
            .registry_declared_version
            .as_deref()
            .unwrap_or("none declared"),
    );
    let _ = writeln!(
        out,
        "registry digest {}",
        document.registry_digest.as_deref().unwrap_or("none"),
    );
    let missing = document.acquisition.missing();
    if document.acquisition_complete {
        let _ = writeln!(out, "acquisition complete");
    } else {
        let _ = writeln!(
            out,
            "acquisition incomplete, so this result cannot be declared to match another lab's. Missing: {}",
            missing.join(", "),
        );
    }
    let _ = writeln!(out, "trial {}", document.trial.name);
    out.push('\n');

    for bound in &document.bound_methods {
        let _ = writeln!(
            out,
            "{}{}",
            bound.method_id,
            if bound.registry_backed {
                ""
            } else {
                "  (no registry row carries this id)"
            },
        );
        for (name, value) in &bound.bound_parameters {
            let source = bound
                .parameter_sources
                .get(name)
                .map(|source| source.wire_name())
                .unwrap_or("unrecorded");
            let _ = writeln!(out, "  {name} = {value} ({source})");
        }
        if let Some(sample) = bound.placed_by_hand_at_sample {
            let _ = writeln!(out, "  placed by hand at sample {sample}");
        }
        if let Some(preset) = &bound.preset {
            let _ = writeln!(out, "  adopted from the {} pipeline", preset.id);
        }
    }
    for global in &document.bound_globals {
        let _ = writeln!(
            out,
            "{} = {} {} ({})",
            global.name,
            global.value,
            global.unit_symbol,
            global.source.wire_name(),
        );
    }
    let _ = writeln!(out, "```");
}

/// A run that produced no result, as the answer it is.
///
/// A refusal is what the software has to say about that trial, so it pastes as prose a reader
/// can hand to somebody rather than as the record's JSON envelope. Without this the copy button
/// put `{"refusal":{"code":...}}` on the clipboard, which is the record, not the answer.
pub fn refusal(declined: &plateforce_core::Refusal) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# plateforce declined\n");
    let _ = writeln!(out, "{}\n", escape(declined.message()));
    let _ = writeln!(out, "```");
    let _ = writeln!(out, "code {}", declined.code.wire_name());
    if !declined.method_id.is_empty() {
        let _ = writeln!(out, "rule {}", declined.method_id);
    }
    let _ = writeln!(out, "```");
    out
}

/// A pipe inside a cell would end the column, and a backtick would open a span the rest of the
/// row closes somewhere unintended. Nothing else in a registry label needs escaping.
fn escape(text: &str) -> String {
    text.replace('|', "\\|").replace('`', "'")
}
