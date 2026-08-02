//! One canonical name per quantity, spelled the same on every surface, and every rule
//! filed under a construct the registry declares.
//!
//! The manifest, the Python getters, the R columns and the browser all publish whichever
//! key arrives, so two rules naming one quantity two ways become two quantities the day
//! they land, and no later reader can tell them apart from two different measurements.

use plateforce_analysis::response::QUANTITIES;
use plateforce_analysis::BINDINGS;

use crate::common::registry;

/// The declaration this level was written against. A guard whose subject shrank below the
/// population it was written for would pass by having less to read.
const QUANTITIES_WHEN_WRITTEN: usize = 11;
const BINDINGS_WHEN_WRITTEN: usize = 12;

/// A key states its own unit, so a number cannot cross a surface boundary and be read in a
/// different one. The exception is a quantity the field names without its unit, and the
/// name has to be the registry's own for the construct the arithmetic is filed under
/// rather than one a rule invented.
#[test]
fn every_reported_quantity_carries_its_unit_or_the_registry_name_for_its_construct() {
    let registry = registry();

    let mut by_unit = 0usize;
    let mut by_construct_name = 0usize;
    let mut unnamed = Vec::new();

    for quantity in QUANTITIES {
        if quantity.key.ends_with(quantity.unit) {
            by_unit += 1;
            continue;
        }
        let construct = quantity
            .computed_by
            .and_then(|id| registry.methods.get(id))
            .map(|method| method.construct.as_str());
        match construct {
            Some(construct)
                if quantity.key == construct
                    || quantity.key.starts_with(&format!("{construct}_")) =>
            {
                by_construct_name += 1
            }
            Some(construct) => unnamed.push(format!(
                "{} is in {} and is filed under {construct}",
                quantity.key, quantity.unit
            )),
            None => unnamed.push(format!(
                "{} is in {} and names no arithmetic to take a name from",
                quantity.key, quantity.unit
            )),
        }
    }

    assert!(
        unnamed.is_empty(),
        "{} of {} reported quantities carry neither their unit nor the registry's name for \
         what they measure:\n  {}",
        unnamed.len(),
        QUANTITIES.len(),
        unnamed.join("\n  ")
    );
    assert_eq!(
        by_unit + by_construct_name,
        QUANTITIES.len(),
        "the check read fewer quantities than are declared"
    );
    assert!(
        QUANTITIES.len() >= QUANTITIES_WHEN_WRITTEN,
        "{} quantities are declared where this guard was written against {QUANTITIES_WHEN_WRITTEN}",
        QUANTITIES.len()
    );
}

/// Two rules that report one quantity are compared by holding the key still and letting
/// `computed_by` vary. A key that carries the rule instead makes them two quantities.
#[test]
fn no_two_quantities_share_a_key_and_no_key_carries_a_registry_id() {
    let mut keys: Vec<&str> = QUANTITIES.iter().map(|quantity| quantity.key).collect();
    let declared = keys.len();
    keys.sort_unstable();
    keys.dedup();

    assert_eq!(
        keys.len(),
        declared,
        "{} of {declared} declared quantities share a key with another",
        declared - keys.len()
    );

    let dotted: Vec<&str> = QUANTITIES
        .iter()
        .map(|quantity| quantity.key)
        .filter(|key| key.contains('.'))
        .collect();
    assert!(
        dotted.is_empty(),
        "{} of {declared} keys carry a dotted id, so the rule has taken the quantity's place \
         in the name: {dotted:?}",
        dotted.len()
    );
}

/// A named arithmetic that resolves nowhere leaves the reader unable to look up what
/// produced the number, which is the same as not naming it.
#[test]
fn every_arithmetic_a_quantity_names_resolves_in_the_registry() {
    let registry = registry();

    let mut named = 0usize;
    let mut unresolved = Vec::new();
    for quantity in QUANTITIES {
        let Some(id) = quantity.computed_by else {
            continue;
        };
        named += 1;
        if !registry.methods.contains_key(id) {
            unresolved.push(format!("{} names {id}", quantity.key));
        }
    }

    assert!(
        unresolved.is_empty(),
        "{} of {named} named computations do not resolve, over {} declared quantities:\n  {}",
        unresolved.len(),
        QUANTITIES.len(),
        unresolved.join("\n  ")
    );
    assert!(
        named >= 7,
        "only {named} of {} quantities name the arithmetic behind them",
        QUANTITIES.len()
    );
}

/// The construct is what the binding table is keyed on, so a rule filed under a name the
/// registry does not declare is reachable through no construct and offered on no surface.
#[test]
fn every_construct_a_rule_is_filed_under_is_declared_in_the_registry() {
    let registry = registry();

    let mut undeclared = Vec::new();
    for binding in BINDINGS {
        if !registry.constructs.contains_key(binding.construct) {
            undeclared.push(format!(
                "{} is filed under {}",
                binding.id, binding.construct
            ));
        }
    }

    assert!(
        undeclared.is_empty(),
        "{} of {} rules are filed under a construct the registry does not declare:\n  {}",
        undeclared.len(),
        BINDINGS.len(),
        undeclared.join("\n  ")
    );
    assert!(
        BINDINGS.len() >= BINDINGS_WHEN_WRITTEN,
        "{} rules are bound where this guard was written against {BINDINGS_WHEN_WRITTEN}",
        BINDINGS.len()
    );
}
