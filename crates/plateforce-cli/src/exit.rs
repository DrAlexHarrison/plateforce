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
    /// The three statuses the serve path also returns are read from the crate that returns
    /// them rather than written again here, so the two cannot drift apart.
    pub fn code(self) -> u8 {
        match self {
            Fault::Request => plateforce_serve::A_REQUEST_THAT_CANNOT_BE_HONOURED, // EX_USAGE
            Fault::Recording => 65,                                                // EX_DATAERR
            Fault::Input => 66,                                                    // EX_NOINPUT
            Fault::Internal => plateforce_serve::AN_INVARIANT_THIS_SOFTWARE_BREAKS, // EX_SOFTWARE
            Fault::Registry => 78,                                                 // EX_CONFIG
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
}

impl Outcome {
    /// A complete result with nothing declined.
    pub fn complete(document: String) -> Self {
        Self {
            document: Some(document),
            refusals: Vec::new(),
            fault: None,
        }
    }

    /// Nothing computed, and the record of why.
    pub fn declined(declined: Declined) -> Self {
        Self {
            document: None,
            fault: Some(declined.fault()),
            refusals: vec![declined],
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

/// Whether a result reached the caller, which is the whole of what a status reports.
///
/// A quantity that declined for a reason in the recording is a fact about the recording and
/// travels in the result: in place in the text, in `refusals` in the document, and in
/// `refusals.csv` for a folder. It is not a failure of the run, and most real collections
/// carry one, because a trial trimmed before the athlete lands supports no flight time. A
/// status saying otherwise stops a `&&` chain and a `set -e` script on a run that worked.
pub fn code_for(outcome: &Outcome) -> u8 {
    if outcome.document.is_some() {
        return 0;
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
            (Fault::Input, 66),
            (Fault::Internal, 70),
            (Fault::Registry, 78),
        ];
        for (fault, expected) in mapped {
            assert_eq!(fault.code(), expected, "{fault:?}");
        }
        println!(
            "faults mapped to a sysexits code: {} of {}",
            mapped.len(),
            mapped.len()
        );
    }

    /// Every status the serve path can return is a fault this table names, and names once.
    ///
    /// The two crates used to write these numbers twice, so renumbering the table here would
    /// have left `serve` returning the old ones with nothing to say they had parted. Reading
    /// them rather than restating them is what makes that unrepresentable, and this asserts
    /// the reading reaches every one of them rather than only the pair that prompted it.
    #[test]
    fn every_status_the_serve_path_returns_is_a_fault_this_table_names() {
        let returned = [
            plateforce_serve::A_REQUEST_THAT_CANNOT_BE_HONOURED,
            plateforce_serve::A_PORT_ANOTHER_PROGRAM_HOLDS,
            plateforce_serve::AN_INVARIANT_THIS_SOFTWARE_BREAKS,
            plateforce_serve::A_PORT_THIS_PROCESS_MAY_NOT_HAVE,
        ];
        // The two the serve path raises alone must not be a status this shell raises for
        // something else, which is the only way the two sets can now contradict each other.
        let raised_here: Vec<u8> = [
            Fault::Request,
            Fault::Recording,
            Fault::Input,
            Fault::Internal,
            Fault::Registry,
        ]
        .iter()
        .map(|fault| fault.code())
        .collect();
        for status in [
            plateforce_serve::A_PORT_ANOTHER_PROGRAM_HOLDS,
            plateforce_serve::A_PORT_THIS_PROCESS_MAY_NOT_HAVE,
        ] {
            assert!(
                !raised_here.contains(&status),
                "{status} is returned by the serve path and raised here for another reason"
            );
        }
        // The two both paths use are one number read twice rather than two numbers written.
        assert!(raised_here.contains(&plateforce_serve::A_REQUEST_THAT_CANNOT_BE_HONOURED));
        assert!(raised_here.contains(&plateforce_serve::AN_INVARIANT_THIS_SOFTWARE_BREAKS));
        println!(
            "statuses the serve path returns, checked against the {} this shell raises: {}",
            raised_here.len(),
            returned.len()
        );
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
        };
        assert_eq!(
            code_for(&partial),
            0,
            "a quantity that declined is carried in the result, not in the status"
        );
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
