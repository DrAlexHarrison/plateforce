//! The rules from `docs/schema.md`, enforced rather than documented.

use std::fmt;

use crate::schema::*;
use crate::Registry;

#[derive(Debug, Clone, PartialEq)]
pub enum ViolationKind {
    IdNotDotted,
    UnknownConstruct {
        construct: String,
    },
    UnknownDisagreement {
        target: String,
    },
    AsymmetricDisagreement {
        target: String,
    },
    BiasWithoutCriterion,
    DefaultWithoutSource {
        parameter: String,
    },
    RecommendedOnUnobtainedSource {
        citation: String,
    },
    RefuseWithoutRationale,
    FailureWithoutDenominator,
    FailureRateInconsistent {
        stated: f64,
        computed: f64,
    },
    DuplicateId,
    DefaultDeclaredTwice {
        parameter: String,
    },
    DefaultNamesUnknownValue {
        parameter: String,
        key: String,
    },
    NamedValueDeclaredTwice {
        parameter: String,
        key: String,
    },
    ReachQueryOnSettledBoundary {
        boundary: Boundary,
    },
    PresetBindsUnknownMethod {
        preset: String,
        method_id: String,
    },
    PresetBindingConstructMismatch {
        preset: String,
        method_id: String,
        declared: String,
        actual: String,
    },
    PresetWithoutCitation {
        preset: String,
    },
    PresetBindsOneConstructTwice {
        preset: String,
        construct: String,
    },
    PresetSilentAboutUnknownConstruct {
        preset: String,
        construct: String,
    },
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
            DefaultDeclaredTwice { parameter } => write!(
                f,
                "{}: parameter '{parameter}' declares both default and default_key, so two values claim to be the one the software binds",
                self.entry
            ),
            DefaultNamesUnknownValue { parameter, key } => write!(
                f,
                "{}: parameter '{parameter}' defaults to '{key}', which is not among the values it lists",
                self.entry
            ),
            NamedValueDeclaredTwice { parameter, key } => write!(
                f,
                "{}: parameter '{parameter}' lists '{key}' more than once, so one option replaced another",
                self.entry
            ),
            ReachQueryOnSettledBoundary { boundary } => write!(
                f,
                "{}: reach is '{boundary}' and carries a query, which reads as doubt about a boundary that was settled",
                self.entry
            ),
            RecommendedOnUnobtainedSource { citation } => write!(
                f,
                "{}: status is recommended but rests on '{citation}', which was never obtained",
                self.entry
            ),
            RefuseWithoutRationale => write!(
                f,
                "{}: surfacing is refuse with no gui.rationale, so an interface reads the refusal and cannot say what it is for",
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
            PresetBindsUnknownMethod { preset, method_id } => write!(
                f,
                "{preset}: binds '{method_id}', which the registry does not carry"
            ),
            PresetBindingConstructMismatch {
                preset,
                method_id,
                declared,
                actual,
            } => write!(
                f,
                "{preset}: binds '{method_id}' under construct '{declared}', and that entry's construct is '{actual}'"
            ),
            PresetWithoutCitation { preset } => {
                write!(f, "{preset}: states a pipeline and cites no source for it")
            }
            PresetBindsOneConstructTwice { preset, construct } => write!(
                f,
                "{preset}: binds construct '{construct}' more than once, so one binding replaced another"
            ),
            PresetSilentAboutUnknownConstruct { preset, construct } => write!(
                f,
                "{preset}: states its source says nothing about '{construct}', which is not in constructs.toml"
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
            if parameter.has_default() && parameter.default_source.is_none() {
                violations.push(Violation {
                    entry: entry.clone(),
                    kind: ViolationKind::DefaultWithoutSource {
                        parameter: parameter.name.clone(),
                    },
                });
            }
            if parameter.default.is_some() && parameter.default_key.is_some() {
                violations.push(Violation {
                    entry: entry.clone(),
                    kind: ViolationKind::DefaultDeclaredTwice {
                        parameter: parameter.name.clone(),
                    },
                });
            }

            let mut listed: Vec<&str> = Vec::new();
            for value in &parameter.named_values {
                if listed.contains(&value.key.as_str()) {
                    violations.push(Violation {
                        entry: entry.clone(),
                        kind: ViolationKind::NamedValueDeclaredTwice {
                            parameter: parameter.name.clone(),
                            key: value.key.clone(),
                        },
                    });
                }
                listed.push(&value.key);
            }

            if let Some(key) = &parameter.default_key {
                if !listed.contains(&key.as_str()) {
                    violations.push(Violation {
                        entry: entry.clone(),
                        kind: ViolationKind::DefaultNamesUnknownValue {
                            parameter: parameter.name.clone(),
                            key: key.clone(),
                        },
                    });
                }
            }
        }

        // A query beside a settled boundary is the classification arguing with itself.
        if let Some(reach) = &method.reach {
            let settled = reach.boundary != Boundary::Undetermined;
            let asked = reach
                .query
                .as_ref()
                .is_some_and(|query| !query.trim().is_empty());
            if settled && asked {
                violations.push(Violation {
                    entry: entry.clone(),
                    kind: ViolationKind::ReachQueryOnSettledBoundary {
                        boundary: reach.boundary,
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

        // Every other verdict decides its own behaviour. `refuse` decides only that the
        // rule is not offered, so what a reader is owed instead lives in the rationale.
        if let Some(gui) = &method.gui {
            let unexplained = gui
                .rationale
                .as_ref()
                .is_none_or(|rationale| rationale.trim().is_empty());
            if gui.surfacing == Surfacing::Refuse && unexplained {
                violations.push(Violation {
                    entry: entry.clone(),
                    kind: ViolationKind::RefuseWithoutRationale,
                });
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

    // The preset population's own rules, run here so the registry's validator is one
    // question rather than one per population.
    violations.extend(crate::preset::validate(registry));

    violations
}
