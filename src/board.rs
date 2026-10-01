//! The playfield: occupancy, placement, line clears, and the win/loss checks.
//!
//! The grid carries [`crate::config::HIDDEN_ROWS`] rows above the visible
//! playfield so pieces can spawn and rotate out of sight.

use rand::Rng;

use crate::config::{GRID_HEIGHT, GRID_WIDTH, HIDDEN_ROWS};
use crate::pieces::{cells, Piece, Rotation};

/// Total rows stored, including the hidden buffer.
pub const TOTAL_ROWS: usize = GRID_HEIGHT + HIDDEN_ROWS;

/// The playfield. Each cell is a 1-based colour index, or `0` for empty.
#[derive(Clone)]
pub struct Grid {
    cells: Vec<[u8; GRID_WIDTH]>,
}

impl Grid {
    pub fn new() -> Self {
        Self {
            cells: vec![[0; GRID_WIDTH]; TOTAL_ROWS],
        }
    }

    pub fn reset(&mut self) {
        self.cells.iter_mut().for_each(|row| row.fill(0));
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || x >= GRID_WIDTH as i32 || y < 0 || y >= TOTAL_ROWS as i32 {
            return 0;
        }
        self.cells[y as usize][x as usize]
    }

    #[inline]
    fn set(&mut self, x: i32, y: i32, v: u8) {
        if x < 0 || x >= GRID_WIDTH as i32 || y < 0 || y >= TOTAL_ROWS as i32 {
            return;
        }
        self.cells[y as usize][x as usize] = v;
    }

    /// Write a cell directly, ignoring collisions. Primarily for tests that
    /// need to build a specific position (the original's row editor games
    /// reuse the same shape vocabulary).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn force_set(&mut self, x: i32, y: i32, v: u8) {
        self.set(x, y, v);
    }

    /// Re-colour every placed cell to a random 1..=7 value, empties left
    /// untouched. This drives the original's level-up transition, which
    /// flashed the whole stack through random colours every 100 ms for 2 s
    /// (only the colour indices change — game logic reads them as "not
    /// empty", so play is unaffected).
    pub fn randomize_colors(&mut self) {
        let mut rng = rand::thread_rng();
        for row in self.cells.iter_mut() {
            for c in row.iter_mut() {
                if *c != 0 {
                    *c = rng.gen_range(1..=7);
                }
            }
        }
    }

    /// Empty a single cell.
    pub fn clear_cell(&mut self, x: i32, y: i32) {
        self.set(x, y, 0);
    }

    /// Is this cell inside the legal play area?
    ///
    /// Rows above the grid are legal too: a piece spawns partly above the
    /// ceiling and slides down into view, so the window extends from
    /// `-HIDDEN_ROWS` up to the floor.
    #[inline]
    fn in_bounds(x: i32, y: i32) -> bool {
        x >= 0
            && x < GRID_WIDTH as i32
            && y >= -(HIDDEN_ROWS as i32)
            && y < TOTAL_ROWS as i32
    }

    /// Can a piece sit at this origin in this rotation?
    pub fn fits(&self, piece: Piece, rot: Rotation, origin: (i32, i32)) -> bool {
        let (ox, oy) = origin;
        for (cx, cy) in cells(piece, rot) {
            let x = ox + cx;
            let y = oy + cy;
            if !Self::in_bounds(x, y) {
                return false;
            }
            if y >= 0 && self.cells[y as usize][x as usize] != 0 {
                return false;
            }
        }
        true
    }

    /// Lock a piece into the grid, writing its colour into each occupied cell.
    pub fn place(&mut self, piece: Piece, rot: Rotation, origin: (i32, i32), color: u8) {
        let (ox, oy) = origin;
        for (cx, cy) in cells(piece, rot) {
            self.set(ox + cx, oy + cy, color);
        }
    }

    /// How far can this piece fall from its current origin?
    pub fn drop_distance(&self, piece: Piece, rot: Rotation, origin: (i32, i32)) -> i32 {
        let mut d = 0;
        while self.fits(piece, rot, (origin.0, origin.1 + d + 1)) {
            d += 1;
        }
        d
    }

    /// How exposed a cell is: `1.0` for a block standing free, `0.0` for one
    /// buried in the middle of the stack.
    ///
    /// Counted over the eight neighbours, because the four orthogonal ones
    /// alone make a block on a flat surface read as half-buried — every block
    /// in a filled row has empty space above and below it. The diagonals are
    /// what distinguish "sitting on top of the stack" from "wedged into it".
    ///
    /// The cell's own contents are ignored, so an empty cell sitting in a hole
    /// reports its neighbours' exposure. That is deliberate: the transition asks
    /// how surrounded a cell is, which is a fact about the space, not about
    /// whether anything is currently drawn there.
    pub fn exposure(&self, x: i32, y: i32) -> f32 {
        if x < 0 || x >= GRID_WIDTH as i32 || y < 0 || y >= TOTAL_ROWS as i32 {
            return 1.0;
        }
        let mut open = 0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (x + dx, y + dy);
                // Outside the board is open sky: a block against the wall is on
                // the surface of the stack, not sealed inside it.
                let outside = nx < 0
                    || nx >= GRID_WIDTH as i32
                    || ny < 0
                    || ny >= TOTAL_ROWS as i32;
                if outside || self.get(nx, ny) == 0 {
                    open += 1;
                }
            }
        }
        open as f32 / 8.0
    }

    /// The lowest and highest occupied rows, bottom-up inclusive, or `None` for
    /// an empty board.
    ///
    /// Used to place a block within its stack so the transition has a vertical
    /// coordinate to work with. Scanning costs 33 `is_empty` checks and is done
    /// once per frame rather than per cell.
    pub fn occupied_span(&self) -> Option<(usize, usize)> {
        let mut lo = None;
        let mut hi = None;
        for y in 0..TOTAL_ROWS {
            if self.cells[y].iter().any(|&c| c != 0) {
                lo.get_or_insert(y);
                hi = Some(y);
            }
        }
        Some((lo?, hi?))
    }

    /// Indices of every completely filled row, top to bottom.
    pub fn full_rows(&self) -> Vec<usize> {
        (0..TOTAL_ROWS)
            .filter(|&y| self.cells[y].iter().all(|&c| c != 0))
            .collect()
    }

    /// Remove all full rows and push fresh empty rows in at the top.
    /// Returns the number of rows cleared.
    pub fn clear_lines(&mut self) -> usize {
        let full = self.full_rows();
        if full.is_empty() {
            return 0;
        }
        let full_set: std::collections::HashSet<usize> = full.iter().copied().collect();
        let mut kept: Vec<[u8; GRID_WIDTH]> = Vec::with_capacity(TOTAL_ROWS);
        for (y, row) in self.cells.iter().enumerate() {
            if !full_set.contains(&y) {
                kept.push(*row);
            }
        }
        let cleared = full.len();
        for _ in 0..cleared {
            kept.insert(0, [0; GRID_WIDTH]);
        }
        self.cells = kept;
        cleared
    }

    /// Is the board completely empty?
    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(|row| row.iter().all(|&c| c == 0))
    }

    /// Are the top `n` visible rows occupied? Drives the danger warning.
    pub fn danger_zone_active(&self, n: usize) -> bool {
        (0..n).any(|i| {
            let y = HIDDEN_ROWS + i;
            y < TOTAL_ROWS && self.cells[y].iter().any(|&c| c != 0)
        })
    }

    /// Highest occupied row, in grid coordinates. `None` if the board is empty.
    pub fn highest_occupied_row(&self) -> Option<usize> {
        (0..TOTAL_ROWS).rev().find(|&y| self.cells[y].iter().any(|&c| c != 0))
    }

    /// Has the player topped out?
    ///
    /// The original checked `grid[0]`, the very top of a 31-row board, which
    /// is far above where pieces actually land. This instead reports a top-out
    /// when the stack has grown into the hidden spawn area above the visible
    /// playfield, which is the only way a freshly spawned piece has nowhere
    /// to go. (A piece that locks entirely above the board is a lock-out and
    /// is handled by the game loop.)
    pub fn topped_out(&self) -> bool {
        self.highest_occupied_row()
            .is_some_and(|top| top < HIDDEN_ROWS)
    }

    /// The SRS three-corner T-spin test.
    ///
    /// Looks at the four cells diagonally around the T's centre block; the
    /// spin counts if at least three are blocked, counting the walls and
    /// floor as blocked. Works for every rotation, which the original's
    /// spawn-shape comparison did not.
    pub fn is_t_spin(&self, piece: Piece, _rot: Rotation, origin: (i32, i32)) -> bool {
        if piece != Piece::T {
            return false;
        }
        // Every T rotation has (1, 1) as its centre cell.
        let (cx, cy) = (origin.0 + 1, origin.1 + 1);
        let blocked = |x: i32, y: i32| -> bool {
            if x < 0 || x >= GRID_WIDTH as i32 || y >= TOTAL_ROWS as i32 {
                return true; // wall or floor
            }
            if y < 0 {
                return false; // open sky is not a block
            }
            self.cells[y as usize][x as usize] != 0
        };
        let corners = [
            blocked(cx - 1, cy - 1),
            blocked(cx + 1, cy - 1),
            blocked(cx - 1, cy + 1),
            blocked(cx + 1, cy + 1),
        ];
        corners.iter().filter(|&&b| b).count() >= 3
    }

    /// Snapshot for tests and replays.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn as_rows(&self) -> Vec<Vec<u8>> {
        self.cells.iter().map(|r| r.to_vec()).collect()
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pieces::{spawn_offset, spawn_y};

    fn origin_of(p: Piece) -> (i32, i32) {
        (spawn_offset(p), spawn_y())
    }

    #[test]
    fn new_board_is_empty() {
        let mut g = Grid::new();
        assert!(g.is_empty());
        assert_eq!(g.clear_lines(), 0);
    }

    // --- exposure / occupied_span -----------------------------------------
    //
    // Both exist only to feed the level-up drain, and both are read once per
    // settled block per frame of a two-second transition. A wrong answer here
    // does not crash: it produces a stack that drains in a way nobody intended,
    // which is the failure mode that is hardest to notice and easiest to
    // reintroduce, so the shapes are pinned explicitly.

    /// Both ends of the range, because the drain's whole job is the difference
    /// between them.
    #[test]
    fn exposure_spans_its_full_range() {
        let mut g = Grid::new();
        // Alone on an empty board: every one of the eight neighbours is open, so
        // this is the maximum, not merely "high".
        assert_eq!(g.exposure(7, 20), 1.0, "an isolated cell is fully exposed");

        // Wall it in from all eight sides, one cell clear of every edge so no
        // neighbour is off-board and silently counted as open.
        for dy in -1..=1i32 {
            for dx in -1..=1i32 {
                if dx != 0 || dy != 0 {
                    g.force_set(7 + dx, 20 + dy, 1);
                }
            }
        }
        assert_eq!(g.exposure(7, 20), 0.0, "a sealed cell has no exposure");
    }

    /// Off-board is open sky, not a wall. This is the decision that keeps
    /// `Depth` from treating the entire bottom row of a full board as buried:
    /// a block sitting on the floor is on the surface of the stack, and the
    /// player reads it there.
    #[test]
    fn the_edge_of_the_board_reads_as_open() {
        let mut g = Grid::new();
        for x in 0..GRID_WIDTH as i32 {
            for y in 18..=22 {
                g.force_set(x, y, 1);
            }
        }
        // A block on the floor, hard against the left wall: three of its eight
        // neighbours are off-board and must count as open.
        let corner = g.exposure(0, 18);
        assert!(
            corner > 0.0,
            "a corner block reported {corner} exposure, i.e. fully buried"
        );
        // And a block in the middle of that same solid is buried, which is the
        // contrast the drain is reading.
        assert!(g.exposure(7, 20) < corner);
    }

    /// The eight-neighbour count is the load-bearing detail. On a solid block,
    /// the four orthogonal neighbours alone put a block in the middle of a flat
    /// surface at 0.5 — "half buried" — for a block that is entirely in view.
    /// Including the diagonals is what separates *resting on* the stack from
    /// *wedged into* it.
    #[test]
    fn a_block_on_a_flat_surface_reads_as_exposed() {
        let mut g = Grid::new();
        let mid = GRID_WIDTH as i32 / 2;
        for x in 0..GRID_WIDTH as i32 {
            g.force_set(x, 20, 1);
        }
        // Of the eight neighbours of a mid-row block: two in its own row are
        // filled, the other six are open air above and below.
        assert!((g.exposure(mid, 20) - 0.75).abs() < 1e-6);
        // The orthogonal-only count would be 0.5, i.e. indistinguishable from a
        // block genuinely half-buried. If this ever drops to 0.5 the diagonals
        // have stopped being counted and `Depth` starts eating the surface.
        assert!(
            g.exposure(mid, 20) > 0.5,
            "a block lying on top of the stack is being read as half-buried"
        );
    }

    /// Reads a hole the same way whether or not anything is in it. The drain asks
    /// how enclosed a *cell* is, which is a fact about the space — and the
    /// renderer only ever asks about cells it has already decided are occupied,
    /// so the distinction cannot be observed there, only here.
    #[test]
    fn exposure_ignores_the_cell_its_own_contents() {
        let mut g = Grid::new();
        for dy in -1..=1i32 {
            for dx in -1..=1i32 {
                if dx != 0 || dy != 0 {
                    g.force_set(7 + dx, 20 + dy, 1);
                }
            }
        }
        let hole = g.exposure(7, 20);
        g.force_set(7, 20, 3);
        assert_eq!(g.exposure(7, 20), hole);
    }

    /// Anything the transition can ask, in any state, has to be a real fraction.
    /// Out-of-range coordinates return `1.0` rather than trapping or wrapping, so
    /// a `Wave` frame that runs one row past the stack still draws.
    #[test]
    fn exposure_is_always_a_real_fraction() {
        let g = Grid::new();
        for x in 0..GRID_WIDTH as i32 {
            for y in 0..TOTAL_ROWS as i32 {
                assert!(
                    (0.0..=1.0).contains(&g.exposure(x, y)),
                    "exposure({x}, {y}) = {}",
                    g.exposure(x, y)
                );
            }
        }
        // Well outside the board in every direction.
        for (x, y) in [
            (-1, 20),
            (GRID_WIDTH as i32, 20),
            (7, -1),
            (7, TOTAL_ROWS as i32),
            (i32::MIN, i32::MIN),
            (i32::MAX, i32::MAX),
        ] {
            assert_eq!(g.exposure(x, y), 1.0, "off-board at ({x}, {y})");
        }
    }

    #[test]
    fn an_empty_board_has_no_span() {
        assert_eq!(Grid::new().occupied_span(), None);
    }

    /// The span has to be the *actual* extent, because `Wave` normalises a
    /// block's height against it. A span that ran from row 0 regardless would
    /// compress every real stack into the top few percent of the ramp, and the
    /// band would cross the board in a single frame.
    #[test]
    fn the_span_covers_exactly_what_is_stacked() {
        let mut g = Grid::new();
        g.force_set(3, 20, 1);
        assert_eq!(g.occupied_span(), Some((20, 20)));
        g.force_set(9, 25, 2);
        assert_eq!(g.occupied_span(), Some((20, 25)), "span must widen");
        g.force_set(0, 15, 1);
        assert_eq!(
            g.occupied_span(),
            Some((15, 25)),
            "span must reach down to the new floor"
        );
        // A gap in the middle does not split the span: the stack is one thing
        // with a hole in it, and the wave crosses the hole.
        g.force_set(0, 18, 0);
        assert_eq!(g.occupied_span(), Some((15, 25)));
    }

    /// The hidden spawn rows are part of the stack as far as the transition is
    /// concerned. `render::draw_board` skips them when drawing, but the rows
    /// above the visible field can hold locked cells after a line clear, and a
    /// span that ignored them would report a height the board does not have.
    #[test]
    fn the_span_accounts_for_the_hidden_rows() {
        let mut g = Grid::new();
        g.force_set(4, 20, 1);
        assert_eq!(g.occupied_span(), Some((20, 20)));
        // Row 0 is inside the hidden buffer and must still count as the floor.
        g.force_set(4, 0, 1);
        assert_eq!(g.occupied_span(), Some((0, 20)));
    }

    #[test]
    fn pieces_fit_at_spawn() {
        for p in crate::pieces::ALL_PIECES {
            assert!(Grid::new().fits(p, 0, origin_of(p)), "{p:?}");
        }
    }

    #[test]
    fn walls_block_horizontal_movement() {
        let g = Grid::new();
        for p in crate::pieces::ALL_PIECES {
            // Slide as far left as possible; the first column that stops us is
            // the wall, and exactly one more column to the right is legal.
            let mut x = 0;
            while g.fits(p, 0, (x, spawn_y())) {
                x -= 1;
            }
            assert!(x < 0, "{p:?} never reached the left wall");
            assert!(g.fits(p, 0, (x + 1, spawn_y())));
            assert!(!g.fits(p, 0, (x, spawn_y())));

            // Same on the right: the mirror-image boundary.
            let mut x = 0;
            while g.fits(p, 0, (x, spawn_y())) {
                x += 1;
            }
            assert!(x > 0, "{p:?} never reached the right wall");
            assert!(g.fits(p, 0, (x - 1, spawn_y())));
            assert!(!g.fits(p, 0, (x, spawn_y())));
        }
    }

    #[test]
    fn every_piece_falls_to_rest_on_the_floor() {
        let g = Grid::new();
        for p in crate::pieces::ALL_PIECES {
            let origin = origin_of(p);
            let d = g.drop_distance(p, 0, origin);
            assert!(d > 0, "{p:?} could not fall at all");
            let landed = (origin.0, origin.1 + d);
            assert!(g.fits(p, 0, landed), "{p:?} came to rest out of bounds");
            // One row further is the floor.
            assert!(
                !g.fits(p, 0, (landed.0, landed.1 + 1)),
                "{p:?} sank through the floor"
            );
            // The lowest cell must sit on the bottom row.
            let lowest = cells(p, 0).iter().map(|(_, cy)| *cy).max().unwrap();
            assert_eq!(landed.1 + lowest, (TOTAL_ROWS - 1) as i32);
        }
    }

    #[test]
    fn placement_then_clearing_a_full_row() {
        let mut g = Grid::new();
        // Fill the bottom visible row except the last cell.
        let y = (TOTAL_ROWS - 1) as i32;
        for x in 0..(GRID_WIDTH as i32 - 1) {
            g.set(x, y, 1);
        }
        assert!(g.full_rows().is_empty());
        // Drop an I piece vertically is awkward; just place an O into the gap
        // region and verify the row completes.
        g.set(GRID_WIDTH as i32 - 1, y, 1);
        assert_eq!(g.full_rows(), vec![TOTAL_ROWS - 1]);
        assert_eq!(g.clear_lines(), 1);
        assert!(g.is_empty());
    }

    #[test]
    fn clearing_shifts_rows_down_and_pads_top() {
        let mut g = Grid::new();
        // Two full rows and, beneath them, one marked row that stays put.
        let bottom = TOTAL_ROWS - 1;
        for y in [bottom, bottom - 1] {
            for x in 0..GRID_WIDTH {
                g.set(x as i32, y as i32, 3);
            }
        }
        // The marked row is NOT full: column 0 is left open so it survives
        // the clear, with a distinctive colour in the rest of the row.
        for x in 1..GRID_WIDTH {
            g.set(x as i32, (bottom - 2) as i32, 5);
        }
        assert_eq!(g.clear_lines(), 2);
        // The marked row moved down by two; the top two rows are fresh.
        let marked: Vec<usize> = (0..TOTAL_ROWS).filter(|&y| g.cells[y][1] == 5).collect();
        assert_eq!(marked, vec![TOTAL_ROWS - 1]);
        assert!(g.cells[0].iter().all(|&c| c == 0));
        assert!(g.cells[1].iter().all(|&c| c == 0));
    }

    #[test]
    fn tetris_clears_four_rows_at_once() {
        let mut g = Grid::new();
        let bottom = TOTAL_ROWS;
        for y in bottom - 4..bottom {
            for x in 0..GRID_WIDTH {
                g.set(x as i32, y as i32, 7);
            }
        }
        assert_eq!(g.clear_lines(), 4);
        assert!(g.is_empty());
    }

    #[test]
    fn empty_board_is_not_topped_out() {
        let g = Grid::new();
        assert!(!g.topped_out());
    }

    #[test]
    fn stack_in_the_buffer_tops_out() {
        let mut g = Grid::new();
        for x in 0..GRID_WIDTH {
            g.set(x as i32, 0, 1); // hidden spawn row
        }
        assert!(g.topped_out());
    }

    #[test]
    fn stack_sitting_in_the_upper_buffer_row_tops_out() {
        let mut g = Grid::new();
        // Row 1 is still above the visible field but inside the stored grid.
        for x in 0..GRID_WIDTH {
            g.set(x as i32, 1, 1);
        }
        assert!(g.topped_out());
    }

    /// Corner coordinates of the T's centre block, in the order the
    /// three-corner rule counts them.
    fn corners(origin: (i32, i32)) -> [(i32, i32); 4] {
        let (cx, cy) = (origin.0 + 1, origin.1 + 1);
        [
            (cx - 1, cy - 1),
            (cx + 1, cy - 1),
            (cx - 1, cy + 1),
            (cx + 1, cy + 1),
        ]
    }

    #[test]
    fn t_spin_needs_three_blocked_corners() {
        let g = Grid::new();
        let o = (5, 10);
        let c = corners(o);

        // Open field: no spin.
        assert!(!g.is_t_spin(Piece::T, 0, o));

        // Any two blocked is not enough, whichever two.
        for i in 0..4 {
            for j in (i + 1)..4 {
                let mut g = Grid::new();
                g.set(c[i].0, c[i].1, 1);
                g.set(c[j].0, c[j].1, 1);
                assert!(
                    !g.is_t_spin(Piece::T, 0, o),
                    "two blocked corners {i},{j} were treated as a spin"
                );
            }
        }

        // Any three is a spin.
        for skip in 0..4 {
            let mut g = Grid::new();
            for (i, &(x, y)) in c.iter().enumerate() {
                if i != skip {
                    g.set(x, y, 1);
                }
            }
            assert!(
                g.is_t_spin(Piece::T, 0, o),
                "three blocked corners (missing {skip}) were not a spin"
            );
        }
    }

    #[test]
    fn t_spin_detection_works_in_every_rotation() {
        // This is the case the original missed entirely.
        for rot in 0..4 {
            let mut g = Grid::new();
            let o = (5, 10);
            let c = (o.0 + 1, o.1 + 1);
            g.set(c.0 - 1, c.1 - 1, 1);
            g.set(c.0 + 1, c.1 - 1, 1);
            g.set(c.0 - 1, c.1 + 1, 1);
            assert!(
                g.is_t_spin(Piece::T, rot, o),
                "T-spin not detected in rotation {rot}"
            );
        }
    }

    #[test]
    fn non_t_pieces_are_never_t_spins() {
        let g = Grid::new();
        for p in crate::pieces::ALL_PIECES {
            if p == Piece::T {
                continue;
            }
            assert!(!g.is_t_spin(p, 0, (5, 10)));
        }
    }

    #[test]
    fn the_floor_counts_as_blocked_corners() {
        // A T resting on the floor has its two lower corners on the far side
        // of the floor, which is every bit as blocking as a stack of blocks.
        let mut g = Grid::new();
        let o = (5, (TOTAL_ROWS - 2) as i32);
        assert!(g.fits(Piece::T, 0, o), "test fixture should be a valid rest");
        let c = corners(o);
        // The two bottom corners are floor-blocked, but that alone is not a
        // spin.
        assert!(!g.is_t_spin(Piece::T, 0, o));
        // One more blocked corner and it is.
        g.set(c[0].0, c[0].1, 1);
        assert!(g.is_t_spin(Piece::T, 0, o));
    }

    #[test]
    fn open_sky_above_the_board_does_not_count_as_blocked() {
        let mut g = Grid::new();
        // A T high enough that its top corners are above the ceiling. The sky
        // is not a block, so this must not register as a spin.
        let o = (5, 0);
        let c = corners(o);
        g.set(c[1].0, c[1].1, 1);
        g.set(c[3].0, c[3].1, 1);
        assert!(!g.is_t_spin(Piece::T, 0, o));
    }

    #[test]
    fn danger_zone_reports_high_stacks() {
        let mut g = Grid::new();
        assert!(!g.danger_zone_active(4));
        g.set(5, HIDDEN_ROWS as i32, 1);
        assert!(g.danger_zone_active(4));
    }

    #[test]
    fn randomize_colors_keeps_empties_and_replaces_filled() {
        let mut g = Grid::new();
        g.set(3, 3, 5);
        g.set(7, 8, 2);
        // Rows spoil the fixture if they stay full — place away from edges.
        g.randomize_colors();
        assert_eq!(g.get(0, 0), 0, "empty cells must stay empty");
        let a = g.get(3, 3);
        let b = g.get(7, 8);
        assert!((1..=7).contains(&a), "filled cell recoloured out of range: {a}");
        assert!((1..=7).contains(&b), "filled cell recoloured out of range: {b}");
        // The rest of the board is untouched by the re-colouring.
        assert_eq!(g.get(4, 4), 0);
    }
}
