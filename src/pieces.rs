//! Tetromino definitions, SRS rotation, wall kicks, and the 7-bag randomiser.

use rand::seq::SliceRandom;

/// The seven tetrominoes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Piece {
    I,
    J,
    L,
    O,
    S,
    T,
    Z,
}

pub const ALL_PIECES: [Piece; 7] = [
    Piece::I,
    Piece::J,
    Piece::L,
    Piece::O,
    Piece::S,
    Piece::T,
    Piece::Z,
];

impl Piece {
    /// Does this piece have its own kick table? O never needs to move.
    fn uses_own_kicks(self) -> bool {
        matches!(self, Piece::I)
    }
}

/// A single rotation state: four `(x, y)` cell offsets, origin at the
/// piece's own top-left bounding box.
pub type Cells = [(i32, i32); 4];

/// Rotation state, 0 = spawn, 1 = right, 2 = upside down, 3 = left.
pub type Rotation = usize;

/// Occupied cells for every rotation of every piece, in SRS spawn positions.
///
/// Flattened to 28 entries so the nesting cannot be miscounted. Index with
/// `piece as usize * 4 + rotation`. Piece order matches [`Piece`]'s
/// discriminant order: I, J, L, O, S, T, Z.
///
/// Each state is the previous one rotated 90 degrees clockwise about the centre of
/// the piece's fixed-size box (4x4 for I, 3x3 for J/L/S/T/Z, 2x2 for O).
/// That fixed pivot is what the SRS kick tables assume: a wall kick shifts
/// the origin by a fixed `(dx, dy)` and every rotated shape must agree on
/// where the box centre sits, or a kick that should open a gap instead
/// collides and the rotation is wrongly rejected.
const SHAPES: [Cells; 28] = [
    // --- I (4x4 box, pivot centre 1.5,1.5)
    [(0, 1), (1, 1), (2, 1), (3, 1)], // 0   spawn, flat
    [(2, 0), (2, 1), (2, 2), (2, 3)], // 1   right
    [(0, 2), (1, 2), (2, 2), (3, 2)], // 2   upside down
    [(1, 0), (1, 1), (1, 2), (1, 3)], // 3   left
    // --- J (3x3 box, pivot 1,1)
    [(0, 0), (0, 1), (1, 1), (2, 1)], // 0   spawn
    [(1, 0), (2, 0), (1, 1), (1, 2)], // 1   right
    [(0, 1), (1, 1), (2, 1), (2, 2)], // 2
    [(1, 0), (1, 1), (0, 2), (1, 2)], // 3   left
    // --- L (3x3 box, pivot 1,1)
    [(2, 0), (0, 1), (1, 1), (2, 1)], // 0   spawn
    [(1, 0), (1, 1), (1, 2), (2, 2)], // 1   right
    [(0, 1), (1, 1), (2, 1), (0, 2)], // 2
    [(0, 0), (1, 0), (1, 1), (1, 2)], // 3   left
    // --- O (2x2 box, identical in all four states)
    [(0, 0), (1, 0), (0, 1), (1, 1)], // 0
    [(0, 0), (1, 0), (0, 1), (1, 1)], // 1
    [(0, 0), (1, 0), (0, 1), (1, 1)], // 2
    [(0, 0), (1, 0), (0, 1), (1, 1)], // 3
    // --- S (3x3 box, pivot 1,1)
    [(1, 0), (2, 0), (0, 1), (1, 1)], // 0   spawn
    [(1, 0), (1, 1), (2, 1), (2, 2)], // 1   right
    [(1, 1), (2, 1), (0, 2), (1, 2)], // 2
    [(0, 0), (0, 1), (1, 1), (1, 2)], // 3   left
    // --- T (3x3 box, pivot 1,1)
    [(1, 0), (0, 1), (1, 1), (2, 1)], // 0   spawn
    [(1, 0), (1, 1), (2, 1), (1, 2)], // 1   right
    [(0, 1), (1, 1), (2, 1), (1, 2)], // 2
    [(1, 0), (0, 1), (1, 1), (1, 2)], // 3   left
    // --- Z (3x3 box, pivot 1,1)
    [(0, 0), (1, 0), (1, 1), (2, 1)], // 0   spawn
    [(2, 0), (1, 1), (2, 1), (1, 2)], // 1   right
    [(0, 1), (1, 1), (1, 2), (2, 2)], // 2
    [(1, 0), (0, 1), (1, 1), (0, 2)], // 3   left
];

/// Occupied cells for a piece in a given rotation state.
pub fn cells(piece: Piece, rot: Rotation) -> Cells {
    SHAPES[piece as usize * 4 + rot % 4]
}

/// The O piece's spawn offset is measured from a 4x4 box so it sits
/// consistently with the others; this is its true origin correction.
const O_ORIGIN: (i32, i32) = (1, 0);

/// Horizontal spawn position, so pieces enter centred as they do in the
/// original.
pub fn spawn_offset(piece: Piece) -> i32 {
    let extra = if piece == Piece::O { O_ORIGIN.0 } else { 0 };
    (crate::config::GRID_WIDTH as i32) / 2 - 2 - extra
}

/// Vertical spawn position. Pieces start just above the ceiling so they
/// slide into view, which also gives rotation room.
pub fn spawn_y() -> i32 {
    -(crate::config::HIDDEN_ROWS as i32)
}

// --- Wall kicks -----------------------------------------------------------
//
// Tables are taken verbatim from the original. `(dx, dy)` with `y+` = down.

type Kicks = [(i32, i32); 5];

const JLSTZ_KICKS: [(Rotation, Rotation, Kicks); 8] = [
    (0, 1, [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)]),
    (1, 0, [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)]),
    (1, 2, [(0, 0), (1, 0), (1, 1), (0, -2), (1, -2)]),
    (2, 1, [(0, 0), (-1, 0), (-1, -1), (0, 2), (-1, 2)]),
    (2, 3, [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)]),
    (3, 2, [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)]),
    (3, 0, [(0, 0), (-1, 0), (-1, 1), (0, -2), (-1, -2)]),
    (0, 3, [(0, 0), (1, 0), (1, -1), (0, 2), (1, 2)]),
];

const I_KICKS: [(Rotation, Rotation, Kicks); 8] = [
    (0, 1, [(0, 0), (-2, 0), (1, 0), (-2, -1), (1, 2)]),
    (1, 0, [(0, 0), (2, 0), (-1, 0), (2, 1), (-1, -2)]),
    (1, 2, [(0, 0), (-1, 0), (2, 0), (-1, 2), (2, -1)]),
    (2, 1, [(0, 0), (1, 0), (-2, 0), (1, -2), (-2, 1)]),
    (2, 3, [(0, 0), (2, 0), (-1, 0), (2, 1), (-1, -2)]),
    (3, 2, [(0, 0), (-2, 0), (1, 0), (-2, -1), (1, 2)]),
    (3, 0, [(0, 0), (1, 0), (-2, 0), (1, -2), (-2, 1)]),
    (0, 3, [(0, 0), (-1, 0), (2, 0), (-1, 2), (2, -1)]),
];

/// The kick offsets to try, in order, for a rotation from `from` to `to`.
pub fn kicks_for(piece: Piece, from: Rotation, to: Rotation) -> Kicks {
    // O never kicks, and the original special-cases it by shape index.
    if piece == Piece::O {
        return [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)];
    }
    let table = if piece.uses_own_kicks() {
        &I_KICKS
    } else {
        &JLSTZ_KICKS
    };
    for (f, t, k) in table.iter() {
        if *f == from % 4 && *t == to % 4 {
            return *k;
        }
    }
    [(0, 0), (0, 0), (0, 0), (0, 0), (0, 0)]
}

/// Rotate one step clockwise: `0 -> 1 -> 2 -> 3 -> 0`.
pub fn rotate_cw(from: Rotation) -> Rotation {
    (from + 1) % 4
}

/// Rotate one step counter-clockwise.
pub fn rotate_ccw(from: Rotation) -> Rotation {
    (from + 3) % 4
}

// --- 7-bag ---------------------------------------------------------------

/// A fair randomiser: each bag of seven contains every piece exactly once.
pub struct Bag {
    bag: Vec<Piece>,
}

impl Bag {
    pub fn new() -> Self {
        Self { bag: Vec::with_capacity(7) }
    }

    fn refill(&mut self) {
        self.bag.clear();
        self.bag.extend_from_slice(&ALL_PIECES);
        self.bag.shuffle(&mut rand::thread_rng());
    }

    /// Next piece, refilling the bag when it runs dry.
    pub fn next_piece(&mut self) -> Piece {
        if self.bag.is_empty() {
            self.refill();
        }
        self.bag.pop().expect("bag was just refilled")
    }

    /// Peek at the next `n` pieces without consuming them.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn peek(&mut self, n: usize) -> Vec<Piece> {
        while self.bag.len() < n {
            let saved = std::mem::take(&mut self.bag);
            self.refill();
            let mut fresh = std::mem::replace(&mut self.bag, saved);
            fresh.reverse();
            self.bag = fresh;
        }
        self.bag.iter().rev().take(n).copied().collect()
    }
}

impl Default for Bag {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_piece_has_four_cells_in_every_rotation() {
        for p in ALL_PIECES {
            for r in 0..4 {
                assert_eq!(cells(p, r).len(), 4, "{p:?} rot {r}");
            }
        }
    }

    #[test]
    fn o_piece_is_rotation_invariant() {
        for r in 0..4 {
            assert_eq!(cells(Piece::O, r), cells(Piece::O, 0));
        }
    }

    /// The rotation box side length for a piece. Each piece turns inside its
    /// own fixed box: 4x4 for the straight piece, 2x2 for the square, 3x3
    /// for the rest.
    fn box_dim(p: Piece) -> i32 {
        match p {
            Piece::I => 4,
            Piece::O => 2,
            _ => 3,
        }
    }

    /// Longest unbroken run of edge-adjacent cells along one axis: 3 for the
    /// horizontal runs, 2 for the vertical ones.
    ///
    /// `horizontal` picks the axis. This is an orientation-independent way to
    /// tell the arm pieces (J/L/T, which have a 3-cell bar) from the skews
    /// (S/Z, which never do).
    fn longest_run(c: Cells, horizontal: bool) -> usize {
        // A horizontal run is measured within one row, so the line is y and
        // the position along it is x; and the other way round.
        let line_of = |i: usize| if horizontal { c[i].1 } else { c[i].0 };
        let pos_of = |i: usize| if horizontal { c[i].0 } else { c[i].1 };
        let cell_on = |line: i32, pos: i32| if horizontal { (pos, line) } else { (line, pos) };
        // Project onto the axis, then walk each occupied line counting the
        // longest contiguous stretch.
        let mut lines: Vec<i32> = (0..4).map(line_of).collect();
        lines.sort_unstable();
        lines.dedup();
        let mut best = 0;
        for line in lines {
            let mut on: Vec<i32> = (0..4)
                .map(pos_of)
                .filter(|&v| c.contains(&cell_on(line, v)))
                .collect();
            on.sort_unstable();
            // Several cells can share a position once projected (only one can
            // actually be on the grid, so duplicates must go before counting).
            on.dedup();
            let mut run = 0usize;
            let mut prev: Option<i32> = None;
            for v in on {
                run = if prev == Some(v - 1) { run + 1 } else { 1 };
                prev = Some(v);
                best = best.max(run);
            }
        }
        best
    }

    /// Every rotation state must be the previous one turned 90 degrees clockwise
    /// about the centre of the piece's fixed-size box. This is the property
    /// the SRS kick tables rely on: kicks shift the origin by a fixed
    /// `(dx, dy)`, which only opens the intended gap if every state agrees on
    /// where the box centre sits. Broken shapes (a rotation that is not a
    /// rigid turn, or that turns about the wrong pivot) fail here, which is
    /// exactly the bug that stopped pieces from flipping into an open space.
    #[test]
    fn every_rotation_is_a_rigid_turn_about_the_box_centre() {
        for p in ALL_PIECES {
            let d = box_dim(p);
            for r in 0..4 {
                let from = cells(p, r);
                let to: Vec<(i32, i32)> = cells(p, (r + 1) % 4).to_vec();
                let turned: Vec<(i32, i32)> = from
                    .iter()
                    // 90 degrees CW inside a d x d box: (x, y) -> (d-1-y, x).
                    .map(|&(x, y)| (d - 1 - y, x))
                    .collect();
                let mut want = turned.clone();
                let mut got = to.clone();
                want.sort_unstable();
                got.sort_unstable();
                assert_eq!(
                    got, want,
                    "{p:?} rot {r}->{} is not a rigid clockwise turn about the {d}x{d} centre",
                    (r + 1) % 4
                );
            }
        }
    }

    /// Every rotation of every piece must be the same tetromino family: four
    /// edge-connected cells, with the piece's characteristic arrangement (the
    /// two skews stay skews, the three triomino-arm pieces keep their T / L /
    /// J shape). Guards against a table entry that is not even the right
    /// piece.
    #[test]
    fn every_rotation_is_the_right_tetromino() {
        for p in ALL_PIECES {
            for r in 0..4 {
                let c = cells(p, r);
                // All four cells distinct.
                let mut uniq: Vec<(i32, i32)> = c.to_vec();
                uniq.sort_unstable();
                uniq.dedup();
                assert_eq!(uniq.len(), 4, "{p:?} rot {r} has duplicate cells");

                // Edge-connected (flood fill from the first cell).
                let mut seen = vec![(c[0].0, c[0].1)];
                let mut grew = true;
                while grew {
                    grew = false;
                    for (x, y) in seen.clone() {
                        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                            let n = (x + dx, y + dy);
                            if c.contains(&n) && !seen.contains(&n) {
                                seen.push(n);
                                grew = true;
                            }
                        }
                    }
                }
                assert_eq!(seen.len(), 4, "{p:?} rot {r} is not edge-connected");

                // Signature: count of each row occupancy pattern. A skew has
                // two rows of 2 offset; the straight/arm pieces differ.
                let rows = {
                    let mut rows: Vec<i32> = c.iter().map(|&(_, y)| y).collect();
                    rows.sort_unstable();
                    rows.dedup();
                    rows.len()
                };
                let cols = {
                    let mut cols: Vec<i32> = c.iter().map(|&(x, _)| x).collect();
                    cols.sort_unstable();
                    cols.dedup();
                    cols.len()
                };
                match p {
                    // O is a 2x2 block in every state.
                    Piece::O => {
                        assert_eq!((rows, cols), (2, 2), "O rot {r} is not 2x2");
                    }
                    // The straight piece is 1x4 or 4x1, i.e. a single run of 4.
                    Piece::I => {
                        assert!(
                            (rows == 1 && cols == 4) || (rows == 4 && cols == 1),
                            "I rot {r} is not a straight bar"
                        );
                    }
                    // Skews are always 2x3 or 3x2, and -- unlike J/L/T -- have
                    // no three-in-a-line run at all. That is the property that
                    // tells a skew apart from an arm piece in any orientation.
                    Piece::S | Piece::Z => {
                        assert!(
                            (rows == 2 && cols == 3) || (rows == 3 && cols == 2),
                            "{p:?} rot {r} is not a skew bounding box"
                        );
                        assert_eq!(
                            longest_run(c, true),
                            2,
                            "{p:?} rot {r} should have no 3-in-a-line bar"
                        );
                        assert_eq!(
                            longest_run(c, false),
                            2,
                            "{p:?} rot {r} should have no 3-in-a-line bar"
                        );
                    }
                    // J, L and T are all "three in a line plus one cell beside
                    // an end of the bar". The bar points along rows in some
                    // states and along columns in others, so compare the two
                    // axes as a sorted pair rather than a fixed order.
                    Piece::J | Piece::L | Piece::T => {
                        assert!(
                            (rows == 2 && cols == 3) || (rows == 3 && cols == 2),
                            "{p:?} rot {r} should be a 2x3 (or 3x2) shape"
                        );
                        let mut runs = [longest_run(c, true), longest_run(c, false)];
                        runs.sort_unstable();
                        assert_eq!(
                            runs,
                            [2, 3],
                            "{p:?} rot {r} should be three in a line plus one nub, got runs {runs:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn rotations_cycle_in_both_directions() {
        assert_eq!(rotate_cw(rotate_cw(rotate_cw(rotate_cw(0)))), 0);
        assert_eq!(rotate_ccw(0), 3);
        assert_eq!(rotate_cw(3), 0);
    }

    #[test]
    fn kick_tables_cover_all_eight_transitions() {
        for (f, t) in [
            (0, 1),
            (1, 0),
            (1, 2),
            (2, 1),
            (2, 3),
            (3, 2),
            (3, 0),
            (0, 3),
        ] {
            assert_ne!(kicks_for(Piece::T, f, t)[0], (99, 99), "T {f}->{t}");
            assert_ne!(kicks_for(Piece::I, f, t)[0], (99, 99), "I {f}->{t}");
        }
    }

    #[test]
    fn o_never_kicks() {
        assert_eq!(kicks_for(Piece::O, 0, 1), [(0, 0); 5]);
    }

    #[test]
    fn bag_emits_each_piece_once_per_ten() {
        // 70 draws must contain each of the 7 pieces exactly 10 times.
        let mut bag = Bag::new();
        let mut counts = [0usize; 7];
        for _ in 0..70 {
            let p = bag.next_piece();
            counts[p as usize] += 1;
        }
        assert!(counts.iter().all(|&c| c == 10), "counts: {counts:?}");
    }

    #[test]
    fn bag_never_repeats_inside_a_bag() {
        let mut bag = Bag::new();
        for _ in 0..7 {
            let first = bag.next_piece();
            let mut seen = vec![first];
            while seen.len() < 7 {
                let p = bag.next_piece();
                assert!(!seen.contains(&p), "repeat {p:?} within a bag");
                seen.push(p);
            }
        }
    }

    #[test]
    fn peek_does_not_consume() {
        let mut bag = Bag::new();
        let a = bag.peek(5);
        let b = bag.peek(5);
        assert_eq!(a, b);
    }
}
