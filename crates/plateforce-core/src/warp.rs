//! Monotone time warping of one curve onto another.
//!
//! Registration needs no landmarks, which is its whole appeal, and that is also its risk: a
//! warp free to stretch as far as it likes will align features that are not the same
//! feature, and the result looks clean. A smeared average at least looks smeared. So the
//! warping function comes back with the aligned curve rather than being consumed inside it,
//! and the amount of warping is reported as a number beside it.
//!
//! Cost is the product of the two lengths, so this is set-level work rather than something
//! that runs while a marker is being dragged.
//!
//! Nothing here decides a method. A caller passes the band a bound rule resolved.

/// One curve mapped onto another, and the map itself.
#[derive(Debug, Clone, PartialEq)]
pub struct Warp {
    /// Every matched pair, query index first, in order. Monotone in both coordinates by
    /// construction, which is what stops the alignment reordering the movement.
    pub matched_pairs: Vec<(usize, usize)>,
    /// The warping function: for each query sample, the reference position it was carried
    /// to. This is the thing that gets plotted, because a warp nobody looked at is the
    /// failure mode the rule carries.
    pub reference_position_of_query: Vec<f64>,
    pub total_cost: f64,
    /// The largest distance the warp travelled from the straight line that would leave the
    /// timebase alone, as a fraction of the reference length. Zero is no warping.
    pub greatest_departure_from_no_warping: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum WarpError {
    #[error("warp needs both curves to be non-empty: query {query_length}, reference {reference_length}")]
    EmptyCurve {
        query_length: usize,
        reference_length: usize,
    },
    #[error("warp(band_samples = {band_samples}) is narrower than the {length_difference} samples between the two curve lengths, so no monotone path spans them")]
    BandNarrowerThanLengthDifference {
        band_samples: usize,
        length_difference: usize,
    },
}

/// Align `query` onto `reference` by the cheapest monotone path.
///
/// `band_samples` bounds how far the path may stray from the straight line, which is what
/// keeps a stretch from reaching a feature that is not its counterpart. `None` leaves it
/// unbounded, which the rule permits and its own note warns about.
pub fn align_to_reference(
    query: &[f64],
    reference: &[f64],
    band_samples: Option<usize>,
) -> Result<Warp, WarpError> {
    let (rows, columns) = (query.len(), reference.len());
    if rows == 0 || columns == 0 {
        return Err(WarpError::EmptyCurve {
            query_length: rows,
            reference_length: columns,
        });
    }
    let length_difference = rows.abs_diff(columns);
    if let Some(band) = band_samples {
        if band < length_difference {
            return Err(WarpError::BandNarrowerThanLengthDifference {
                band_samples: band,
                length_difference,
            });
        }
    }

    // The straight line between the two ends, which is where a band is measured from and
    // what "no warping" means when the two curves are different lengths.
    let slope = (columns - 1) as f64 / (rows - 1).max(1) as f64;
    let within_band = |row: usize, column: usize| match band_samples {
        Some(band) => (column as f64 - row as f64 * slope).abs() <= band as f64,
        None => true,
    };

    let mut accumulated = vec![vec![f64::INFINITY; columns]; rows];
    for row in 0..rows {
        for column in 0..columns {
            if !within_band(row, column) {
                continue;
            }
            let step = (query[row] - reference[column]).abs();
            let cheapest = if row == 0 && column == 0 {
                0.0
            } else {
                let mut best = f64::INFINITY;
                if row > 0 {
                    best = best.min(accumulated[row - 1][column]);
                }
                if column > 0 {
                    best = best.min(accumulated[row][column - 1]);
                }
                if row > 0 && column > 0 {
                    best = best.min(accumulated[row - 1][column - 1]);
                }
                best
            };
            accumulated[row][column] = step + cheapest;
        }
    }

    let mut matched_pairs = vec![(rows - 1, columns - 1)];
    let (mut row, mut column) = (rows - 1, columns - 1);
    while row > 0 || column > 0 {
        let diagonal = if row > 0 && column > 0 {
            accumulated[row - 1][column - 1]
        } else {
            f64::INFINITY
        };
        let upward = if row > 0 {
            accumulated[row - 1][column]
        } else {
            f64::INFINITY
        };
        let leftward = if column > 0 {
            accumulated[row][column - 1]
        } else {
            f64::INFINITY
        };
        // Ties go diagonal, so an alignment that needs no stretching does not acquire one.
        if diagonal <= upward && diagonal <= leftward {
            row -= 1;
            column -= 1;
        } else if upward <= leftward {
            row -= 1;
        } else {
            column -= 1;
        }
        matched_pairs.push((row, column));
    }
    matched_pairs.reverse();

    let mut carried_total = vec![0.0f64; rows];
    let mut carried_count = vec![0usize; rows];
    for &(at_row, at_column) in &matched_pairs {
        carried_total[at_row] += at_column as f64;
        carried_count[at_row] += 1;
    }
    let reference_position_of_query: Vec<f64> = carried_total
        .iter()
        .zip(&carried_count)
        .map(|(total, count)| total / (*count).max(1) as f64)
        .collect();

    let greatest_departure_from_no_warping = reference_position_of_query
        .iter()
        .enumerate()
        .map(|(at_row, carried)| (carried - at_row as f64 * slope).abs())
        .fold(0.0f64, f64::max)
        / (columns - 1).max(1) as f64;

    Ok(Warp {
        matched_pairs,
        reference_position_of_query,
        total_cost: accumulated[rows - 1][columns - 1],
        greatest_departure_from_no_warping,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bell(length: usize, centre: f64, width: f64) -> Vec<f64> {
        (0..length)
            .map(|index| {
                let offset = (index as f64 - centre) / width;
                (-offset * offset).exp()
            })
            .collect()
    }

    #[test]
    fn a_curve_aligned_onto_itself_needs_no_warping() {
        let curve = bell(300, 150.0, 30.0);
        let warp = align_to_reference(&curve, &curve, Some(40)).unwrap();
        assert!(warp.total_cost < 1e-12, "cost {}", warp.total_cost);
        assert!(
            warp.greatest_departure_from_no_warping < 1e-12,
            "departure {}",
            warp.greatest_departure_from_no_warping
        );
        for (index, carried) in warp.reference_position_of_query.iter().enumerate() {
            assert!((carried - index as f64).abs() < 1e-9, "sample {index}");
        }
    }

    /// The warping function is monotone, which is the guarantee in the rule's name: an
    /// alignment may stretch the movement and may never reorder it.
    #[test]
    fn the_warping_function_never_runs_backwards() {
        let early = bell(300, 110.0, 25.0);
        let late = bell(300, 190.0, 40.0);
        let warp = align_to_reference(&early, &late, Some(120)).unwrap();
        for pair in warp.reference_position_of_query.windows(2) {
            assert!(pair[1] >= pair[0], "{pair:?} runs backwards");
        }
        for pair in warp.matched_pairs.windows(2) {
            assert!(pair[1].0 >= pair[0].0 && pair[1].1 >= pair[0].1, "{pair:?}");
        }
    }

    /// A shifted feature is what warping is for, and the departure figure is what makes the
    /// stretch visible rather than absorbed.
    #[test]
    fn a_shifted_feature_is_aligned_and_the_stretch_is_reported() {
        let reference = bell(300, 150.0, 25.0);
        let shifted = bell(300, 210.0, 25.0);
        let warp = align_to_reference(&shifted, &reference, Some(120)).unwrap();
        let aligned_cost = warp.total_cost;

        let unaligned: f64 = shifted
            .iter()
            .zip(&reference)
            .map(|(left, right)| (left - right).abs())
            .sum();
        assert!(
            aligned_cost < unaligned / 2.0,
            "warping bought nothing: {aligned_cost} against {unaligned}"
        );
        assert!(
            warp.greatest_departure_from_no_warping > 0.1,
            "a 60 sample shift reported a departure of {}",
            warp.greatest_departure_from_no_warping
        );
    }

    /// The band is the guard against aligning features that are not counterparts, so it has
    /// to actually bind: the same pair of curves costs more under a narrow band than a wide
    /// one, because the narrow one refuses the long stretch.
    #[test]
    fn a_narrow_band_refuses_the_stretch_a_wide_one_allows() {
        let reference = bell(300, 150.0, 25.0);
        let shifted = bell(300, 210.0, 25.0);
        let wide = align_to_reference(&shifted, &reference, Some(120)).unwrap();
        let narrow = align_to_reference(&shifted, &reference, Some(5)).unwrap();
        assert!(
            narrow.total_cost > wide.total_cost,
            "the band did not bind: narrow {} against wide {}",
            narrow.total_cost,
            wide.total_cost
        );
        assert!(
            narrow.greatest_departure_from_no_warping < wide.greatest_departure_from_no_warping
        );
    }

    #[test]
    fn curves_of_different_lengths_align_end_to_end() {
        let reference = bell(400, 200.0, 30.0);
        let query = bell(250, 125.0, 19.0);
        let warp = align_to_reference(&query, &reference, None).unwrap();
        assert_eq!(warp.matched_pairs.first().unwrap(), &(0usize, 0usize));
        assert_eq!(warp.matched_pairs.last().unwrap(), &(249usize, 399usize));
        assert_eq!(warp.reference_position_of_query.len(), 250);
    }

    #[test]
    fn a_band_narrower_than_the_length_difference_names_both_numbers() {
        let reference = bell(400, 200.0, 30.0);
        let query = bell(250, 125.0, 19.0);
        let error = align_to_reference(&query, &reference, Some(10)).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("10"), "{message}");
        assert!(message.contains("150"), "{message}");
    }

    #[test]
    fn an_empty_curve_names_both_lengths() {
        let error = align_to_reference(&[], &[1.0, 2.0], None).unwrap_err();
        let message = error.to_string();
        assert!(message.contains('0'), "{message}");
        assert!(message.contains('2'), "{message}");
    }
}
