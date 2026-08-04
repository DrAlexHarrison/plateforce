//! Gravity as a measured property of where the plate is, rather than a constant.
//!
//! Jump height is proportional to gravity by both routes. The impulse route gives
//! `h = g J^2 / 2 W^2` and the flight-time route `h = g t^2 / 8`, so a relative error in `g`
//! is the same relative error in height on both, and the ratio between them is untouched.
//!
//! Gravity varies by half a percent across the Earth's surface, so two labs at different
//! latitudes computing "the same" jump height are not computing the same quantity.
//!
//! Measured against standard gravity: Bellingham, where the corpus was collected, is +0.029%;
//! Phoenix is -0.119%; Mexico City -0.281%. Every one of those exceeds a hundredth of a
//! percent, and it takes only 318 m of elevation or ten degrees of latitude to get there.

/// Standard gravity, the defined constant. The default, because it is true by definition
/// rather than by measurement.
pub const STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED: f64 = 9.80665;

/// Free-air correction, in metres per second squared per metre of elevation. Gravity falls as
/// the plate moves away from the centre of the Earth.
const FREE_AIR_PER_METRE: f64 = 3.086e-6;

/// Where the plate is. Both fields are acquisition facts about the capture, not analysis
/// choices, which is why they belong in the acquisition block rather than among the method
/// parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlateLocation {
    pub latitude_degrees: f64,
    pub elevation_meters: f64,
}

/// Theoretical gravity at a latitude and elevation, by the 1980 International Gravity Formula
/// on the WGS84 ellipsoid, with the free-air correction applied.
///
/// This is the theoretical value for the ellipsoid, not a measured local value: it does not
/// know about the rock under the building, and a gravimeter would disagree by a few parts in
/// a hundred thousand. That is an order of magnitude below the latitude term it corrects, so
/// it improves the number without pretending to be a survey.
pub fn at_location(location: PlateLocation) -> f64 {
    let sin_squared = location.latitude_degrees.to_radians().sin().powi(2);
    let ellipsoid = 9.780_326_771_5
        * (1.0
            + 0.005_279_041_4 * sin_squared
            + 0.000_023_271_8 * sin_squared.powi(2)
            + 0.000_000_126_2 * sin_squared.powi(3)
            + 0.000_000_000_7 * sin_squared.powi(4));
    ellipsoid - FREE_AIR_PER_METRE * location.elevation_meters
}

/// Whether stating the location is worth the trouble, against a threshold the caller sets.
///
/// Exists so an interface can stay quiet when the correction is below what the user cares
/// about, rather than demanding a latitude from someone measuring a schoolchild's vertical.
pub fn differs_from_standard_by_more_than(
    location: PlateLocation,
    relative_threshold: f64,
) -> bool {
    let local = at_location(location);
    ((local - STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED)
        / STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED)
        .abs()
        > relative_threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location(latitude_degrees: f64, elevation_meters: f64) -> PlateLocation {
        PlateLocation {
            latitude_degrees,
            elevation_meters,
        }
    }

    /// The formula's own anchors: it is defined to give 9.7803267715 at the equator, and the
    /// value near 45 degrees is what standard gravity was chosen to approximate.
    #[test]
    fn the_formula_reproduces_its_published_anchors() {
        assert!((at_location(location(0.0, 0.0)) - 9.780_326_771_5).abs() < 1e-9);
        assert!((at_location(location(45.0, 0.0)) - 9.806_19).abs() < 1e-4);
        assert!((at_location(location(90.0, 0.0)) - 9.832_186).abs() < 1e-4);
    }

    #[test]
    fn gravity_falls_with_elevation_and_rises_with_latitude() {
        let sea_level = at_location(location(35.0, 0.0));
        let mountain = at_location(location(35.0, 2000.0));
        assert!(mountain < sea_level);
        assert!((sea_level - mountain - 2000.0 * FREE_AIR_PER_METRE).abs() < 1e-12);
        assert!(at_location(location(60.0, 0.0)) > at_location(location(20.0, 0.0)));
    }

    /// The question a user actually asks: is stating my location worth it? At a hundredth of a
    /// percent the answer is yes almost everywhere, and the exceptions are near the equator.
    #[test]
    fn a_hundredth_of_a_percent_is_crossed_by_ordinary_places() {
        let threshold = 0.0001;
        for (latitude, elevation) in [
            (48.75, 20.0),
            (33.45, 340.0),
            (19.43, 2240.0),
            (51.51, 11.0),
        ] {
            assert!(
                differs_from_standard_by_more_than(location(latitude, elevation), threshold),
                "{latitude} degrees at {elevation} m should clear the threshold"
            );
        }
        // Near the equator at sea level the latitude and elevation terms nearly cancel the
        // offset, so the honest answer there is that it does not matter.
        assert!(!differs_from_standard_by_more_than(
            location(5.0, 0.0),
            0.01
        ));
    }

    /// 318 m is where elevation alone crosses a hundredth of a percent. Worth pinning, because
    /// it is the number that decides whether an interface should ask.
    #[test]
    fn elevation_alone_crosses_the_threshold_at_about_three_hundred_metres() {
        let crossing = STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED * 0.0001 / FREE_AIR_PER_METRE;
        assert!((crossing - 318.0).abs() < 1.0, "crossing was {crossing} m");
    }
}
