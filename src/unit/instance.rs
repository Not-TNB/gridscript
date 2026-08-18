//! Unit tests for `interpreter::instance`

use super::*;

/* --- geometry --- */

#[test]
fn point_containment() {
    let center = (5.0, 5.0);

    assert!(point_in_circle((5.0, 5.0), center, 1.0)); // dead center
    assert!(point_in_circle((5.5, 5.0), center, 1.0)); // inside
    assert!(point_in_circle((6.0, 5.0), center, 1.0)); // exactly on the edge
    assert!(!point_in_circle((6.1, 5.0), center, 1.0)); // just outside
    assert!(!point_in_circle((9.0, 9.0), center, 1.0)); // far away
}

#[test]
fn segment_through_circle() {
    let center = (5.0, 5.0);
    // horizontal segment passing straight through the center
    assert!(segment_intersects_circle(
        (3.0, 5.0),
        (7.0, 5.0),
        center,
        1.0
    ));
}

#[test]
fn segment_missing_circle() {
    let center = (5.0, 5.0);
    // parallel but too far north
    assert!(!segment_intersects_circle(
        (3.0, 2.0),
        (7.0, 2.0),
        center,
        1.0
    ));
}

#[test]
fn segment_grazing_edge() {
    let center = (5.0, 5.0);
    // passes exactly `radius` away — inclusive, so this counts
    assert!(segment_intersects_circle(
        (3.0, 4.0),
        (7.0, 4.0),
        center,
        1.0
    ));
}

#[test]
fn segment_stops_short_of_circle() {
    // The clamp's reason for existing: the circle lies on the infinite line
    // through this segment, but beyond its end. An unclamped projection would
    // wrongly report a hit here.
    let center = (10.0, 5.0);
    assert!(!segment_intersects_circle(
        (1.0, 5.0),
        (3.0, 5.0),
        center,
        1.0
    ));
}

#[test]
fn segment_starting_inside() {
    let center = (5.0, 5.0);
    // begins within the circle and exits it
    assert!(segment_intersects_circle(
        (5.0, 5.0),
        (8.0, 5.0),
        center,
        1.0
    ));
}

#[test]
fn tiny_circle_is_not_skipped() {
    // The spec's stated motivation for a segment test over endpoint tests: a
    // node smaller than one step's travel must still trigger when crossed.
    let center = (5.0, 5.0);
    let radius = 0.1;
    // neither endpoint is inside, but the segment passes through
    assert!(!point_in_circle((4.0, 5.0), center, radius));
    assert!(!point_in_circle((6.0, 5.0), center, radius));
    assert!(segment_intersects_circle(
        (4.0, 5.0),
        (6.0, 5.0),
        center,
        radius
    ));
}

#[test]
fn degenerate_zero_length_segment() {
    let center = (5.0, 5.0);
    // start == end: falls back to a point test at that position
    assert!(segment_intersects_circle(
        (5.0, 5.0),
        (5.0, 5.0),
        center,
        1.0
    ));
    assert!(!segment_intersects_circle(
        (9.0, 9.0),
        (9.0, 9.0),
        center,
        1.0
    ));
}
