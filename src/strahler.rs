//! The Strahler order of a line network, from its topology alone (no elevation).
//!
//! Port of the `computeStrahler` method of the QGIS plugin
//! (github.com/LucasDaniel28/Strahler3.0, originally by Alexandre Delahaye), written without
//! recursion so that long rivers cannot overflow the WebAssembly stack.
//!
//! The rules, as in the original: headwaters get order 1; where lines of the same order meet,
//! the downstream line gets the next order; where different orders meet, it keeps the largest.

use std::collections::HashMap;

/// A vertex, as its exact coordinates: two segments connect when their vertices are equal.
pub type Key = (u64, u64);

/// The key of the vertex `(x, y)`. `-0.0` is folded into `0.0` so that they compare equal.
pub fn key(x: f64, y: f64) -> Key {
    ((x + 0.0).to_bits(), (y + 0.0).to_bits())
}

/// One segment on the walk: its index, the vertex it was entered from, and what it has seen.
struct Frame {
    index: usize,
    /// The vertex at the far end, where its own children join.
    end: Key,
    /// The segments that touch `end`, other than this one.
    children: Vec<usize>,
    /// How many of `children` have been handed out so far.
    next: usize,
    /// The order of each child, in the order they were handed out.
    orders: Vec<u32>,
}

impl Frame {
    fn new(index: usize, origin: Key, lines: &[Option<[Key; 2]>], at: &HashMap<Key, Vec<usize>>) -> Frame {
        let [a, b] = lines[index].expect("only segments with endpoints are walked");
        let end = if a == origin { b } else { a };
        let children = at.get(&end).map(|touching| touching.iter().copied().filter(|&c| c != index).collect());
        Frame { index, end, children: children.unwrap_or_default(), next: 0, orders: Vec::new() }
    }

    /// The order of this segment, once all its children have one.
    fn order(&self) -> u32 {
        if self.children.is_empty() {
            return 1;
        }
        let max = *self.orders.iter().max().expect("a segment with children has orders");
        let maxima = self.orders.iter().filter(|&&o| o == max).count();
        if maxima == 1 { max } else { max + 1 }
    }
}

/// The segments that touch each vertex.
fn touching(lines: &[Option<[Key; 2]>]) -> HashMap<Key, Vec<usize>> {
    let mut at: HashMap<Key, Vec<usize>> = HashMap::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some([a, b]) = line {
            at.entry(*a).or_default().push(i);
            if b != a {
                at.entry(*b).or_default().push(i);
            }
        }
    }
    at
}

/// The vertex `(x, y)` in degrees, from a key.
fn coords(key: Key) -> (f64, f64) {
    (f64::from_bits(key.0), f64::from_bits(key.1))
}

/// The free end (touched by one segment only) nearest to `(x, y)`, described as `lat,lon`.
fn nearest_free_end(at: &HashMap<Key, Vec<usize>>, x: f64, y: f64) -> String {
    let nearest = at
        .iter()
        .filter(|(_, touch)| touch.len() == 1)
        .map(|(key, _)| {
            let (kx, ky) = coords(*key);
            ((kx - x).hypot(ky - y), *key)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    match nearest {
        Some((_, key)) => {
            let (kx, ky) = coords(key);
            format!("{ky},{kx}")
        }
        None => "none (every segment end is joined)".into(),
    }
}

/// The vertex the outlet point `(x, y)` names: the segment end nearest to it, within `tolerance`
/// degrees, which must be a free end. The point is in the layer's coordinates (degrees).
pub fn outlet_key(lines: &[Option<[Key; 2]>], x: f64, y: f64, tolerance: f64) -> Result<Key, String> {
    let at = touching(lines);
    let (distance, key) = at
        .keys()
        .map(|key| {
            let (kx, ky) = coords(*key);
            ((kx - x).hypot(ky - y), *key)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .ok_or("The layer has no segment ends.")?;

    if distance > tolerance {
        let free = nearest_free_end(&at, x, y);
        return Err(format!("No segment end within {tolerance} degrees of the outlet. The nearest free end is at {free}."));
    }
    if at[&key].len() > 1 {
        let free = nearest_free_end(&at, x, y);
        return Err(format!("The outlet is on a junction, not a free end. The nearest free end is at {free}."));
    }
    Ok(key)
}

/// The Strahler order of every segment of a network, walking upstream from the `outlet`.
///
/// `lines[i]` holds the first and last vertex of segment `i`, or `None` for a segment to skip
/// (multipart geometries). The segment that starts or ends at `outlet` is the root, and its other
/// end must be connected to the network. Segments the walk never reaches get `None`.
pub fn orders(lines: &[Option<[Key; 2]>], outlet: Key) -> Result<Vec<Option<u32>>, String> {
    let at = touching(lines);

    let root = match at.get(&outlet).map(Vec::as_slice) {
        None | Some([]) => return Err("No segment starts or ends at the outlet point.".into()),
        Some([root]) => *root,
        Some(_) => return Err("More than one segment meets at the outlet point; it must be a free end.".into()),
    };

    let root_frame = Frame::new(root, outlet, lines, &at);
    if root_frame.children.is_empty() {
        return Err("The root segment is not connected to a network.".into());
    }

    let mut order = vec![None; lines.len()];
    let mut seen = vec![false; lines.len()];
    seen[root] = true;
    let mut stack = vec![root_frame];

    // Walk upstream, depth first. A child already seen (only possible in a looped network)
    // contributes 0, as in the original.
    loop {
        let Some(top) = stack.last_mut() else { break };
        match top.children.get(top.next).copied() {
            Some(child) => {
                top.next += 1;
                if seen[child] {
                    top.orders.push(0);
                } else {
                    seen[child] = true;
                    let origin = top.end;
                    stack.push(Frame::new(child, origin, lines, &at));
                }
            }
            None => {
                let frame = stack.pop().expect("the loop checked the stack is not empty");
                let value = frame.order();
                order[frame.index] = Some(value);
                if let Some(parent) = stack.last_mut() {
                    parent.orders.push(value);
                }
            }
        }
    }

    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A segment from `(x1, y1)` to `(x2, y2)`.
    fn seg(x1: f64, y1: f64, x2: f64, y2: f64) -> Option<[Key; 2]> {
        Some([key(x1, y1), key(x2, y2)])
    }

    #[test]
    fn a_chain_is_all_order_one() {
        let lines = [seg(0.0, 0.0, 0.0, 1.0), seg(0.0, 1.0, 0.0, 2.0), seg(0.0, 2.0, 0.0, 3.0)];
        assert_eq!(orders(&lines, key(0.0, 0.0)).unwrap(), [Some(1), Some(1), Some(1)]);
    }

    #[test]
    fn two_headwaters_make_order_two() {
        // A Y: the root meets two headwaters of order 1.
        let lines = [seg(0.0, 0.0, 0.0, 1.0), seg(0.0, 1.0, -1.0, 2.0), seg(0.0, 1.0, 1.0, 2.0)];
        assert_eq!(orders(&lines, key(0.0, 0.0)).unwrap(), [Some(2), Some(1), Some(1)]);
    }

    #[test]
    fn a_larger_branch_keeps_its_order() {
        // The root meets a branch of order 2 (segment 1) and a headwater (segment 2): the unique
        // maximum wins, so the root is 2 and the headwater beside the branch's own pair stays 1.
        let lines = [
            seg(0.0, 0.0, 0.0, 1.0),
            seg(0.0, 1.0, -1.0, 2.0),
            seg(0.0, 1.0, 1.0, 2.0),
            seg(-1.0, 2.0, -2.0, 3.0),
            seg(-1.0, 2.0, 0.0, 3.0),
        ];
        assert_eq!(orders(&lines, key(0.0, 0.0)).unwrap(), [Some(2), Some(2), Some(1), Some(1), Some(1)]);
    }

    #[test]
    fn a_junction_of_three_order_ones_makes_order_two() {
        // Three headwaters meeting at one vertex: the maxima are tied, so the parent gets max + 1.
        let lines = [
            seg(0.0, 0.0, 0.0, 1.0),
            seg(0.0, 1.0, -1.0, 2.0),
            seg(0.0, 1.0, 0.0, 2.0),
            seg(0.0, 1.0, 1.0, 2.0),
        ];
        assert_eq!(orders(&lines, key(0.0, 0.0)).unwrap()[0], Some(2));
    }

    #[test]
    fn multipart_segments_are_skipped() {
        let lines = [seg(0.0, 0.0, 0.0, 1.0), None, seg(0.0, 1.0, 1.0, 2.0)];
        assert_eq!(orders(&lines, key(0.0, 0.0)).unwrap(), [Some(1), None, Some(1)]);
    }

    #[test]
    fn the_outlet_must_be_a_free_end_of_one_segment() {
        let lines = [seg(0.0, 0.0, 0.0, 1.0), seg(0.0, 1.0, 1.0, 2.0)];
        assert!(orders(&lines, key(5.0, 5.0)).is_err());
        assert!(orders(&lines, key(0.0, 1.0)).is_err());
    }

    #[test]
    fn an_isolated_root_is_rejected() {
        let lines = [seg(0.0, 0.0, 0.0, 1.0)];
        assert!(orders(&lines, key(0.0, 0.0)).is_err());
    }

    #[test]
    fn an_outlet_near_a_free_end_snaps_to_it() {
        let lines = [seg(0.0, 0.0, 0.0, 1.0), seg(0.0, 1.0, 0.0, 2.0)];
        assert_eq!(outlet_key(&lines, 0.00001, -0.00001, 0.0001), Ok(key(0.0, 0.0)));
    }

    #[test]
    fn an_outlet_on_a_junction_is_rejected_with_the_nearest_free_end() {
        let lines = [seg(0.0, 0.0, 0.0, 1.0), seg(0.0, 1.0, 1.0, 2.0), seg(0.0, 1.0, -1.0, 2.0)];
        let err = outlet_key(&lines, 0.0, 1.0, 0.0001).unwrap_err();
        assert!(err.contains("junction"), "{err}");
        assert!(err.contains("0,0"), "{err}");
    }

    #[test]
    fn an_outlet_far_from_the_network_is_rejected() {
        let lines = [seg(0.0, 0.0, 0.0, 1.0)];
        let err = outlet_key(&lines, 5.0, 5.0, 0.0001).unwrap_err();
        assert!(err.contains("No segment end within"), "{err}");
    }

    #[test]
    fn negative_zero_matches_zero() {
        assert_eq!(key(-0.0, 0.0), key(0.0, 0.0));
    }
}
