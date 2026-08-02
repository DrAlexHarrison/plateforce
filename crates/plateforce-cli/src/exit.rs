//! What the shell learns from a run without reading a sentence, and which stream carries it.

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
            Fault::Internal => 70,  // EX_SOFTWARE
            Fault::Registry => 78,  // EX_CONFIG
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
    /// The rendered result. `None` when nothing computed, and the whole document or nothing:
    /// a caller that has written half a document into this field has already lost the
    /// property that makes stdout parseable.
    pub document: Option<String>,
    /// Sentences from the layer that declined. This crate composes none of them.
    pub refusals: Vec<String>,
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

    /// Nothing computed, and the reason why.
    pub fn declined(fault: Fault, message: String) -> Self {
        Self {
            document: None,
            refusals: vec![message],
            fault: Some(fault),
            every_requested_quantity_has_a_value: false,
        }
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

    #[test]
    fn a_complete_result_exits_zero_and_a_partial_one_reports_the_recording() {
        let complete = Outcome::complete("{}".to_string());
        assert_eq!(code_for(&complete), 0);
        assert_eq!(stream_for(&complete), Stream::Stdout);

        let partial = Outcome {
            document: Some("{}".to_string()),
            refusals: vec!["a rule found no crossing".to_string()],
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
        let refused = Outcome::declined(Fault::Request, "the registry has no entry a.b".into());
        assert_eq!(code_for(&refused), 64);
        assert_eq!(stream_for(&refused), Stream::Stderr);
    }
}
