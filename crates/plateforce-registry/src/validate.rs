//! The eight rules from `docs/schema.md`, enforced rather than documented.

use std::fmt;

use crate::schema::*;
use crate::Registry;

#[derive(Debug, Clone, PartialEq)]
pub enum ViolationKind {
    IdNotDotted,
    UnknownConstruct { construct: String },
    UnknownDisagreement { target: String },
    AsymmetricDisagreement { target: String },
    BiasWithoutCriterion,
    DefaultWithoutSource { parameter: String },
    RecommendedOnUnobtainedSource { citation: String },
    FailureWithoutDenominator,
    FailureRateInconsistent { stated: f64, computed: f64 },
    DuplicateId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Violation {
    pub entry: String,
    pub kind: ViolationKind,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ViolationKind::*;
        match &self.kind {
            IdNotDotted => write!(f, "{}: id is not a dotted canonical name", self.entry),
            DuplicateId => write!(
                f,
                "{}: more than one entry carries this id, so one definition replaced another",
                self.entry
            ),
            UnknownConstruct { construct } => write!(
                f,
                "{}: names construct '{construct}', which is not in constructs.toml",
                self.entry
            ),
            UnknownDisagreement { target } => write!(
                f,
                "{}: disagrees_with '{target}', which does not exist",
                self.entry
            ),
            AsymmetricDisagreement { target } => write!(
                f,
                "{}: disagrees_with '{target}' but '{target}' does not disagree back",
                self.entry
            ),
            BiasWithoutCriterion => write!(
                f,
                "{}: a bias is stated with no criterion, so a reader cannot tell what it is a bias against",
                self.entry
            ),
            DefaultWithoutSource { parameter } => write!(
                f,
                "{}: parameter '{parameter}' has a default with no default_source naming who chose it",
                self.entry
            ),
            RecommendedOnUnobtainedSource { citation } => write!(
                f,
                "{}: status is recommended but rests on '{citation}', which was never obtained",
                self.entry
            ),
            FailureWithoutDenominator => write!(
                f,
                "{}: a failure rate is stated without both numerator and denominator",
                self.entry
            ),
            FailureRateInconsistent { stated, computed } => write!(
                f,
                "{}: failure rate {stated:.4} does not match numerator over denominator, {computed:.4}",
                self.entry
            ),
        }
    }
}

/// Tolerance on a stated failure rate against its own numerator and denominator.
/// Loose enough for a rounded literal, tight enough to catch a transcription error.
const RATE_TOLERANCE: f64 = 0.001;

pub fn validate(registry: &Registry) -> Vec<Violation> {
    let mut violations = Vec::new();

    for method in registry.methods.values() {
        let entry = method.id.clone();

        if !method.id.contains('.') {
            violations.push(Violation {
                entry: entry.clone(),
                kind: ViolationKind::IdNotDotted,
            });
        }
        if !registry.constructs.contains_key(&method.construct) {
            violations.push(Violation {
                entry: entry.clone(),
                kind: ViolationKind::UnknownConstruct {
                    construct: method.construct.clone(),
                },
            });
        }

        for disagreement in &method.disagrees_with {
            match registry.methods.get(&disagreement.id) {
                None => violations.push(Violation {
                    entry: entry.clone(),
                    kind: ViolationKind::UnknownDisagreement {
                        target: disagreement.id.clone(),
                    },
                }),
                Some(other) => {
                    let reciprocated = other.disagrees_with.iter().any(|back| back.id == method.id);
                    if !reciprocated {
                        violations.push(Violation {
                            entry: entry.clone(),
                            kind: ViolationKind::AsymmetricDisagreement {
                                target: disagreement.id.clone(),
                            },
                        });
                    }
                }
            }
        }

        // Serde makes an absent criterion a parse error, so this catches the blank string.
        for bias in &method.biases {
            if bias.criterion.trim().is_empty() {
                violations.push(Violation {
                    entry: entry.clone(),
                    kind: ViolationKind::BiasWithoutCriterion,
                });
            }
        }

        for parameter in &method.parameters {
            if parameter.default.is_some() && parameter.default_source.is_none() {
                violations.push(Violation {
                    entry: entry.clone(),
                    kind: ViolationKind::DefaultWithoutSource {
                        parameter: parameter.name.clone(),
                    },
                });
            }
        }

        if method.status == Status::Recommended {
            for citation in &method.citations {
                let load_bearing = matches!(
                    citation.role,
                    CitationRole::Proposes | CitationRole::Evaluates
                );
                if load_bearing && !citation.obtained {
                    violations.push(Violation {
                        entry: entry.clone(),
                        kind: ViolationKind::RecommendedOnUnobtainedSource {
                            citation: citation.key.clone(),
                        },
                    });
                }
            }
        }

        if let Some(failure) = &method.failure {
            if failure.denominator == 0 {
                violations.push(Violation {
                    entry: entry.clone(),
                    kind: ViolationKind::FailureWithoutDenominator,
                });
            } else {
                let computed = f64::from(failure.numerator) / f64::from(failure.denominator);
                if (computed - failure.rate).abs() > RATE_TOLERANCE {
                    violations.push(Violation {
                        entry: entry.clone(),
                        kind: ViolationKind::FailureRateInconsistent {
                            stated: failure.rate,
                            computed,
                        },
                    });
                }
            }
        }
    }

    for protocol in registry.protocols.values() {
        if !protocol.id.contains('.') {
            violations.push(Violation {
                entry: protocol.id.clone(),
                kind: ViolationKind::IdNotDotted,
            });
        }
        for affected in &protocol.affects {
            let known = registry.constructs.contains_key(affected)
                || registry.methods.contains_key(affected);
            if !known {
                violations.push(Violation {
                    entry: protocol.id.clone(),
                    kind: ViolationKind::UnknownConstruct {
                        construct: affected.clone(),
                    },
                });
            }
        }
    }

    // Ids collide across populations even though the populations are counted apart.
    for id in registry.methods.keys() {
        if registry.protocols.contains_key(id) {
            violations.push(Violation {
                entry: id.clone(),
                kind: ViolationKind::DuplicateId,
            });
        }
    }

    violations
}
