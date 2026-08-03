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
    BiasCriterionUnresolved {
        criterion: String,
    },
    SelfComparisonSweepsNoParameter,
    DefinitionOfRecordCarriesADirection {
        direction: String,
    },
    BiasNamesUnknownParameter {
        parameter: String,
    },
    BiasNamesParameterWithoutDefault {
        parameter: String,
    },
    BiasMagnitudeDisagreesWithParameter {
        parameter: String,
        stated: f64,
        declared: f64,
    },
    BiasUnitDiffersFromParameter {
        parameter: String,
        stated: String,
        declared: String,
    },
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
    ReachUndeterminedWithoutQuery,
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
            BiasCriterionUnresolved { criterion } => write!(
                f,
                "{}: a bias is measured against '{criterion}', which is no entry, no construct and none of the external criteria the vocabulary declares",
                self.entry
            ),
            SelfComparisonSweepsNoParameter => write!(
                f,
                "{}: a bias names this entry as its own criterion under a model comparison, which compares two settings of a parameter this entry does not declare",
                self.entry
            ),
            DefinitionOfRecordCarriesADirection { direction } => write!(
                f,
                "{}: a bias names this entry as its own criterion under a visual comparison, which is the definition of record, and reports it as '{direction}' rather than as the reference's own spread",
                self.entry
            ),
            BiasNamesUnknownParameter { parameter } => write!(
                f,
                "{}: a bias equals parameter '{parameter}', which this entry does not carry",
                self.entry
            ),
            BiasNamesParameterWithoutDefault { parameter } => write!(
                f,
                "{}: a bias equals parameter '{parameter}', which declares no default, so the magnitude is anchored to nothing",
                self.entry
            ),
            BiasMagnitudeDisagreesWithParameter {
                parameter,
                stated,
                declared,
            } => write!(
                f,
                "{}: a bias of {stated} equals parameter '{parameter}', whose default is {declared}",
                self.entry
            ),
            BiasUnitDiffersFromParameter {
                parameter,
                stated,
                declared,
            } => write!(
                f,
                "{}: a bias in {stated} equals parameter '{parameter}', which is in {declared}",
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
            ReachUndeterminedWithoutQuery => write!(
                f,
                "{}: reach is 'undetermined' and carries no query, so it says something stands in the way and not what would settle it",
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

            let criterion = bias.criterion.trim();
            let against_itself = criterion == method.id;
            let resolves = against_itself
                || registry.methods.contains_key(criterion)
                || registry.constructs.contains_key(criterion)
                || EXTERNAL_CRITERIA.contains(&criterion);
            if !criterion.is_empty() && !resolves {
                violations.push(Violation {
                    entry: entry.clone(),
                    kind: ViolationKind::BiasCriterionUnresolved {
                        criterion: bias.criterion.clone(),
                    },
                });
            }

            // An entry naming itself is three claims under one spelling and the kind says which:
            // a model comparison sweeps this entry's own parameter, an instrument comparison is
            // two implementations of it disagreeing, and a visual one is the definition of record,
            // whose figure is the reference's own spread rather than a bias in a direction.
            if against_itself {
                match bias.criterion_kind {
                    CriterionKind::Model if method.parameters.is_empty() => {
                        violations.push(Violation {
                            entry: entry.clone(),
                            kind: ViolationKind::SelfComparisonSweepsNoParameter,
                        });
                    }
                    CriterionKind::HumanVisual => {
                        if let Some(direction) = bias
                            .direction
                            .as_deref()
                            .map(str::trim)
                            .filter(|direction| !matches!(*direction, "none" | "either"))
                        {
                            violations.push(Violation {
                                entry: entry.clone(),
                                kind: ViolationKind::DefinitionOfRecordCarriesADirection {
                                    direction: direction.to_string(),
                                },
                            });
                        }
                    }
                    _ => {}
                }
            }

            // A bias that tracks a parameter is recorded at that parameter's default, so the
            // two are held together here rather than drifting the first time either moves.
            let Some(named) = &bias.equals_parameter else {
                continue;
            };
            let Some(parameter) = method
                .parameters
                .iter()
                .find(|parameter| &parameter.name == named)
            else {
                violations.push(Violation {
                    entry: entry.clone(),
                    kind: ViolationKind::BiasNamesUnknownParameter {
                        parameter: named.clone(),
                    },
                });
                continue;
            };
            match parameter.default {
                None => violations.push(Violation {
                    entry: entry.clone(),
                    kind: ViolationKind::BiasNamesParameterWithoutDefault {
                        parameter: named.clone(),
                    },
                }),
                Some(declared) if declared != bias.magnitude => violations.push(Violation {
                    entry: entry.clone(),
                    kind: ViolationKind::BiasMagnitudeDisagreesWithParameter {
                        parameter: named.clone(),
                        stated: bias.magnitude,
                        declared,
                    },
                }),
                Some(_) => {}
            }
            if parameter.unit.as_deref() != Some(bias.unit.as_str()) {
                violations.push(Violation {
                    entry: entry.clone(),
                    kind: ViolationKind::BiasUnitDiffersFromParameter {
                        parameter: named.clone(),
                        stated: bias.unit.clone(),
                        declared: parameter.unit.clone().unwrap_or_default(),
                    },
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

        // A query beside a settled boundary is the classification arguing with itself, and an
        // undetermined one without a query names a barrier while withholding what would place it.
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
            if !settled && !asked {
                violations.push(Violation {
                    entry: entry.clone(),
                    kind: ViolationKind::ReachUndeterminedWithoutQuery,
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
