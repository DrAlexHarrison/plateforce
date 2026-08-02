//! A rule with no test cannot be told apart from one whose condition became unreachable.
//!
//! Ten rules were in that state at once, which is not a thing a reviewer sees: each was
//! added correctly, and only the whole set read as a gap. So the set is checked here rather
//! than left to whoever adds the next one.

/// The declaration block, so a variant is found by being declared rather than by being used.
fn variants_declared() -> Vec<String> {
    include_str!("../src/validate.rs")
        .split("pub enum ViolationKind {")
        .nth(1)
        .expect("validate.rs declares the violation vocabulary")
        .split("\n}")
        .next()
        .expect("the declaration block closes")
        .lines()
        .filter_map(|line| {
            let named = line.strip_prefix("    ")?;
            let first = named.chars().next()?;
            if !first.is_ascii_uppercase() {
                return None;
            }
            Some(named.trim_end_matches([' ', ',', '{']).trim().to_string())
        })
        .collect()
}

/// Every test file in this directory, named rather than walked, because a path resolved from
/// `CARGO_MANIFEST_DIR` is compiled into the binary and a cached build reads whichever tree
/// it was first built from. A test file added and not named here fails this loudly.
const SUITES: &[&str] = &[
    include_str!("a_preset_is_checked_against_the_registry_it_names_into.rs"),
    include_str!("a_parameter_that_varies_by_name_states_its_options.rs"),
    include_str!("an_entry_out_of_reach_states_what_stands_in_the_way.rs"),
    include_str!("every_rule_the_loader_enforces_fires_when_it_is_broken.rs"),
    include_str!("every_construct_speaks_the_fields_words.rs"),
    include_str!("the_shipped_registry_is_valid.rs"),
];

#[test]
fn every_rule_the_loader_enforces_is_named_by_a_test() {
    let declared = variants_declared();
    assert!(
        declared.len() >= 24,
        "the declaration scan found {} variants, so it is matching nothing: {declared:?}",
        declared.len()
    );

    let untested: Vec<&String> = declared
        .iter()
        .filter(|variant| {
            let named = format!("ViolationKind::{variant}");
            !SUITES.iter().any(|suite| suite.contains(&named))
        })
        .collect();
    assert!(
        untested.is_empty(),
        "{} of {} rules are enforced and named by no test: {untested:?}",
        untested.len(),
        declared.len()
    );
}

/// `docs/schema.md` is the published contract for a format that ships into other people's
/// repositories, and it stated 8 rules while the loader enforced 24. A document asserting a
/// guarantee narrower than the code provides is worse than one asserting a wider guarantee,
/// because a reader writes against the narrow one and is refused by the wide one.
#[test]
fn every_rule_the_loader_enforces_is_named_in_the_published_contract() {
    let contract = include_str!("../../../docs/schema.md");
    let declared = variants_declared();
    let undocumented: Vec<&String> = declared
        .iter()
        .filter(|variant| !contract.contains(variant.as_str()))
        .collect();
    assert!(
        undocumented.is_empty(),
        "{} of {} rules are enforced and absent from docs/schema.md: {undocumented:?}",
        undocumented.len(),
        declared.len()
    );
}

/// The control. Without it a scan that silently matched nothing would report every rule as
/// tested, which is the shape the check above exists to catch one level down.
#[test]
fn the_scan_reads_the_vocabulary_rather_than_an_empty_list() {
    let declared = variants_declared();
    for expected in [
        "IdNotDotted",
        "PresetBindsUnknownMethod",
        "BiasUnitDiffersFromParameter",
        "ReachQueryOnSettledBoundary",
    ] {
        assert!(
            declared.iter().any(|variant| variant == expected),
            "the scan missed {expected}: {declared:?}"
        );
    }
    assert!(
        !declared.iter().any(|variant| variant.contains(':')),
        "the scan picked up a field rather than a variant: {declared:?}"
    );
}
