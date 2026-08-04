//! What the shell learns from a run without reading a sentence, and which stream carries it.

use plateforce_core::{exit_code, Refusal, RefusalCode};
use serde_json::json;

/// The class of fault an exit code reports, one value per `sysexits` code this binary uses.
///
/// The refusal codes the engine raises map onto these four, so a fifth value here would be a
/// fifth exit code rather than a finer reading of an existing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The request asked for something that is not on offer.
    Request,
    /// The recording did not contain what the rule looks for.
    Recording,
    /// The file named could not be read at all, so nothing in it was reached.
    Input,
    /// An invariant this software states and then breaks.
    Internal,
    /// The registry does not load.
    Registry,
}

impl Fault {
    pub fn code(self) -> u8 {
        match self {
            Fault::Request => 64,   // EX_USAGE
            Fault::Recording => 65, // EX_DATAERR
            Fault::Input => 66,     // EX_NOINPUT
            Fault::Internal => 70,  // EX_SOFTWARE
            Fault::Registry => 78,  // EX_CONFIG
        }
    }
}

/// The class a published refusal code belongs to, read off the engine's own table rather than
/// restated here, since a second table would be free to disagree with the first.
pub fn fault_for(code: RefusalCode) -> Fault {
    match exit_code(code) {
        64 => Fault::Request,
        65 => Fault::Recording,
        66 => Fault::Input,
        78 => Fault::Registry,
        _ => Fault::Internal,
    }
}

/// Why a run declined, on its way to a stream.
///
/// A rule or a reader that declined produced a record, and that record is what every surface
/// publishes: a code a caller branches on, the rule, the parameter, and the sentence the
/// engine generated. A fault in the command line reaches no rule and has no such record.
#[derive(Debug, Clone)]
pub struct Declined {
    // Boxed because a `Declined` travels as the error half of a `Result` on the path that
    // builds a request, and a refusal carries every field a caller branches on.
    recorded: Option<Box<Refusal>>,
    fault: Fault,
    terminal: String,
}

impl Declined {
    /// A refusal the engine recorded, shown as the sentence it generated.
    pub fn recorded(refusal: Refusal) -> Self {
        Self {
            fault: fault_for(refusal.code),
            terminal: refusal.message().to_string(),
            recorded: Some(Box::new(refusal)),
        }
    }

    /// A refusal the engine recorded, shown as a layout that says more than its sentence.
    /// The wire still carries the record, so a screen listing candidates and a script reading
    /// a code are reading one refusal.
    pub fn shown_as(refusal: Refusal, terminal: String) -> Self {
        Self {
            fault: fault_for(refusal.code),
            terminal,
            recorded: Some(Box::new(refusal)),
        }
    }

    /// A fault in the line the shell handed over, which no rule reached.
    pub fn line(fault: Fault, message: String) -> Self {
        Self {
            recorded: None,
            fault,
            terminal: message,
        }
    }

    pub fn fault(&self) -> Fault {
        self.fault
    }

    pub fn terminal(&self) -> &str {
        &self.terminal
    }

    /// The record a caller parses, in the shape every surface returns it.
    pub fn record(&self) -> serde_json::Value {
        match &self.recorded {
            Some(refusal) => serde_json::to_value(refusal)
                .unwrap_or_else(|error| json!({ "code": null, "message": format!("{error}") })),
            None => json!({ "code": null, "message": self.terminal }),
        }
    }
}

/// Which stream a subcommand's words go to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// What a subcommand hands back, before anything reaches a stream.
#[derive(Debug, Default)]
pub struct Outcome {
    /// The rendered result. `None` when nothing computed, and the whole document or nothing,
    /// so stdout stays parseable.
    pub document: Option<String>,
    /// Why the run fell short. This crate composes no sentence the engine already has.
    pub refusals: Vec<Declined>,
    pub fault: Option<Fault>,
    pub every_requested_quantity_has_a_value: bool,
}

impl Outcome {
    /// A complete result with nothing declined.
    pub fn complete(document: String) -> Self {
        Self {
            document: Some(document),
            refusals: Vec::new(),
            fault: None,
            every_requested_quantity_has_a_value: true,
        }
    }

    /// Nothing computed, and the record of why.
    pub fn declined(declined: Declined) -> Self {
        Self {
            document: None,
            fault: Some(declined.fault()),
            refusals: vec![declined],
            every_requested_quantity_has_a_value: false,
        }
    }

    /// Nothing computed, and a fault in the line rather than in a rule.
    pub fn declined_line(fault: Fault, message: String) -> Self {
        Self::declined(Declined::line(fault, message))
    }
}

/// A refusal with numbers to sit beside travels with them; one with nothing to sit beside
/// travels alone. Redirecting the numbers to a file must not lose the reason one is absent.
pub fn stream_for(outcome: &Outcome) -> Stream {
    match outcome.document {
        Some(_) => Stream::Stdout,
        None => Stream::Stderr,
    }
}

/// Whether the run gave the caller what it asked for, and where it fell short if not.
///
/// A run that produced a document while one landmark declined answers "no" and says so with
/// the recording's code, whatever the codes inside it: a request the engine could not bind
/// declines before any metric computes, so a document that exists at all was requestable.
pub fn code_for(outcome: &Outcome) -> u8 {
    if outcome.document.is_some() {
        if outcome.every_requested_quantity_has_a_value {
            return 0;
        }
        return Fault::Recording.code();
    }
    outcome.fault.unwrap_or(Fault::Internal).code()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four codes are `sysexits` values and a workflow manager reads them without
    /// parsing a sentence, so a changed number is a changed contract.
    #[test]
    fn every_fault_carries_the_sysexits_code_its_class_names() {
        let mapped = [
            (Fault::Request, 64),
            (Fault::Recording, 65),
            (Fault::Internal, 70),
            (Fault::Registry, 78),
        ];
        for (fault, expected) in mapped {
            assert_eq!(fault.code(), expected, "{fault:?}");
        }
        println!("faults mapped to a sysexits code: {} of 4", mapped.len());
    }

    /// `ALL` is generated beside the enum, so a sixteenth code joins this assertion without an
    /// edit here. A code this shell sorted into `Internal` while the engine gave it 64 would
    /// exit 70 on a request fault.
    #[test]
    fn every_published_code_reaches_the_status_the_engine_gives_it() {
        for code in RefusalCode::ALL {
            assert_eq!(
                i32::from(fault_for(*code).code()),
                exit_code(*code),
                "{code:?}"
            );
        }
        println!(
            "refusal codes whose class carries the engine's own status: {} of {}",
            RefusalCode::ALL.len(),
            RefusalCode::ALL.len()
        );
    }

    #[test]
    fn a_complete_result_exits_zero_and_a_partial_one_reports_the_recording() {
        let complete = Outcome::complete("{}".to_string());
        assert_eq!(code_for(&complete), 0);
        assert_eq!(stream_for(&complete), Stream::Stdout);

        let partial = Outcome {
            document: Some("{}".to_string()),
            refusals: vec![Declined::recorded(Refusal::no_crossing(
                "onset.threshold.noise_relative",
                "k",
                5.0,
                3.0,
            ))],
            fault: Some(Fault::Recording),
            every_requested_quantity_has_a_value: false,
        };
        assert_eq!(code_for(&partial), 65);
        assert_eq!(
            stream_for(&partial),
            Stream::Stdout,
            "a refusal with numbers beside it travels with them"
        );
    }

    #[test]
    fn a_refusal_with_no_result_beside_it_goes_to_the_other_stream() {
        let refused =
            Outcome::declined_line(Fault::Request, "the registry has no entry a.b".to_string());
        assert_eq!(code_for(&refused), 64);
        assert_eq!(stream_for(&refused), Stream::Stderr);
    }

    /// A caller branches on the code, and a fault in the line has none to branch on: a code
    /// invented here would name a failure no other surface can raise.
    #[test]
    fn a_recorded_refusal_publishes_its_code_and_a_line_fault_publishes_none() {
        let recorded = Declined::recorded(Refusal::registry_invalid("methods/onset.toml"));
        assert_eq!(recorded.record()["code"], json!("registry_invalid"));
        assert_eq!(recorded.fault(), Fault::Registry);

        let line = Declined::line(
            Fault::Request,
            "--registry names two directories".to_string(),
        );
        assert_eq!(line.record()["code"], serde_json::Value::Null);
        assert_eq!(
            line.record()["message"],
            json!("--registry names two directories")
        );
    }

    /// The screen shows the candidates, the wire carries the record, and the code is on both.
    #[test]
    fn a_layout_replaces_the_sentence_on_screen_and_never_on_the_wire() {
        let refusal = Refusal::decision_not_made(
            "this result",
            vec!["system_weight".to_string(), "movement_onset".to_string()],
        );
        let sentence = refusal.message().to_string();
        let shown = Declined::shown_as(refusal, "  --weighing <METHOD>\n  --onset <METHOD>".into());
        assert!(shown.terminal().contains("--weighing"));
        assert_eq!(shown.record()["message"], json!(sentence));
        assert_eq!(shown.record()["code"], json!("decision_not_made"));
    }
}
