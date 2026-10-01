//! Layout, timing and palette constants.
//!
//! These mirror `TetraFusion_2.1.py` exactly so the port plays identically.

/// Width of the playfield, in pixels.
pub const SCREEN_WIDTH: i32 = 450;
/// Height of the playfield, in pixels.
pub const SCREEN_HEIGHT: i32 = 930;
/// Edge length of one block, in pixels.
pub const BLOCK_SIZE: i32 = 30;
/// Width of the side panel, in pixels.
pub const SUBWINDOW_WIDTH: i32 = 369;

/// Width of everything the game draws, in layout pixels: the playfield plus the
/// side panel.
///
/// This is the size of the render texture, so it is also the area the background
/// photo has to cover. Defined once here because the background used to be fitted
/// to [`SCREEN_WIDTH`] alone: the photo stopped at the right-hand edge of the well
/// and left the entire panel on the plain backdrop, which made a photo that
/// changed with the level almost impossible to notice.
pub const CONTENT_WIDTH: i32 = SCREEN_WIDTH + SUBWINDOW_WIDTH;

/// Clear space kept around the game, in layout pixels.
///
/// The playfield is `GRID_HEIGHT * BLOCK_SIZE` tall, which is exactly
/// `SCREEN_HEIGHT`, and it is `GRID_WIDTH * BLOCK_SIZE` wide, which is exactly
/// `SCREEN_WIDTH`. Sizing the window to the layout alone therefore puts the
/// board flush against the framebuffer on every side, with the floor landing on
/// the very last pixel row. There is no slack at all: the window's title bar and
/// borders are drawn outside the framebuffer but sit over the top and bottom of
/// what the player can see, so the outermost cells end up visually cropped.
///
/// The window is made `2 * WINDOW_MARGIN` larger than the layout and the margin
/// is carried through [`crate::render::present`], so the game always sits
/// `WINDOW_MARGIN` pixels inside the framebuffer at 1:1 and keeps a proportional
/// border once it is scaled up.
pub const WINDOW_MARGIN: i32 = 24;

/// Board is 15 x 31.
pub const GRID_WIDTH: usize = (SCREEN_WIDTH / BLOCK_SIZE) as usize;
pub const GRID_HEIGHT: usize = (SCREEN_HEIGHT / BLOCK_SIZE) as usize;

/// Rows of buffer above the visible board, so pieces can rotate without
/// popping into existence. Not drawn.
pub const HIDDEN_ROWS: usize = 2;

/// Lock delay window, in milliseconds.
pub const LOCK_DELAY_TIME: u32 = 500;
/// How many times lock delay may be refreshed before it stops resetting.
pub const MAX_LOCK_DELAY_RESETS: u32 = 15;
/// How many times the piece may move or rotate while grounded before
/// lock delay stops resetting. (Same budget as the reset counter.)
pub const MAX_LOCK_DELAY_MOVES: u32 = 15;

/// Floor on gravity so late-game levels stay playable.
///
/// Fifty milliseconds per row is exactly twenty rows per second, i.e. **20G** -
/// the ceiling the whole game is designed around. Nothing is ever allowed to
/// fall faster than this, which is what makes [`MAX_GRAVITY`] a real maximum
/// rather than a number a high difficulty can sail past.
pub const MIN_FALL_SPEED: u32 = 50;

/// Gravity, in G, at the top of the range: pieces fall twenty rows a second.
///
/// One G is one row per second, so the ceiling is `1000 / MAX_GRAVITY` ms per
/// row, which is [`MIN_FALL_SPEED`]. The two are the same number reached two
/// ways and are asserted equal in the tests, because if they ever drift the
/// "20G" the rest of the design talks about stops being what the game does.
pub const MAX_GRAVITY: u32 = 20;

/// Gravity in G at `level`.
///
/// The ramp is linear: one G at level one, [`MAX_GRAVITY`] at level
/// [`MAX_GRAVITY`], and pinned there after. The original's `0.85 ** (level - 1)`
/// curve was much flatter - it needed about twenty-three levels to reach the
/// fifty-millisecond floor, and only got past 3G at level seven, which left a
/// long stretch of a run that did not feel like it was getting harder. Reaching
/// the ceiling exactly at the level named in the design keeps the ramp legible:
/// the player can watch the number they care about arrive on schedule.
///
/// Levels at or below zero are treated as level one rather than being allowed
/// to divide by zero or produce a zero-G freeze.
pub fn gravity_g(level: i32) -> u32 {
    level.clamp(1, MAX_GRAVITY as i32) as u32
}

/// Milliseconds per row at `level`, for a difficulty whose level-one gravity is
/// `base_ms`.
///
/// `base_ms` is the level-one cadence, so `base_ms = 1000` means the difficulty
/// opens at 1G and hits 20G at level 20; a faster difficulty shifts the whole
/// curve up but can never beat the [`MIN_FALL_SPEED`] ceiling.
///
/// Rounded rather than truncated: the value is a cadence, not a reproduction of
/// an integer expression, and a row that lands 0.4 ms early is invisible while
/// a systematic half-row bias is not.
pub fn fall_speed_for(base_ms: u32, level: i32) -> u32 {
    let ms = base_ms as f64 / gravity_g(level) as f64;
    (ms.round() as u32).max(MIN_FALL_SPEED)
}

/// Soft drop cadence in ms per row. The original fell at a fixed
/// `current_fall_speed = 50 if fast_fall else fall_speed`, i.e. twenty rows
/// per second while holding DOWN - not one row per frame.
pub const SOFT_DROP_SPEED: u32 = 50;
/// Points for an ordinary clear of `n` rows at once, times the level.
///
/// The Tetris Guideline table: single 100, double 300, triple 500, Tetris 800.
/// Index is the number of rows cleared, so index 0 is unreachable and kept at
/// zero rather than being a hole a caller can index into.
pub const LINE_CLEAR_POINTS: [i32; 5] = [0, 100, 300, 500, 800];
/// Full T-spin points by rows cleared (0..=3), times the level.
pub const TSPIN_POINTS: [i32; 4] = [400, 800, 1200, 1600];
/// Mini T-spin points by rows cleared (0..=2), times the level.
///
/// A mini with a line clear is worth less than the same clear as a full spin -
/// that is the whole point of telling them apart.
pub const MINI_TSPIN_POINTS: [i32; 3] = [100, 200, 400];
/// Perfect-clear bonus by rows cleared (1..=4), times the level.
pub const PERFECT_CLEAR_POINTS: [i32; 5] = [0, 800, 1200, 1800, 2000];
/// Points per row of hard drop distance.
pub const POINTS_PER_HARD_DROP_ROW: i32 = 2;
/// Points per row descended under a held soft drop.
pub const POINTS_PER_SOFT_DROP_ROW: i32 = 1;
/// Combo award: `50 * combo * level`, added to every clear after the first.
pub const COMBO_BASE: i32 = 50;
/// Back-to-back is a 1.5x multiplier on the clear, expressed as a fraction so
/// the arithmetic stays exact in integer points.
pub const B2B_NUM: i32 = 3;
pub const B2B_DEN: i32 = 2;
/// Rows cleared per level.
pub const LINES_PER_LEVEL: i32 = 10;

/// The port's own version, taken from `Cargo.toml`.
///
/// The version the game prints in the window title and under the main menu
/// used to be a hand typed string literal, so bumping the package version in
/// `Cargo.toml` left the game still announcing 2.1. Reading it from the
/// manifest means there is exactly one place to change it.
/// The subtitle drawn under `TETRAFUSION` on the main menu, for example
/// `2.2.0 rust edition`. `concat!` folds the manifest value in at compile
/// time, so it cannot drift from the package version.
pub const VERSION_LINE: &str = concat!(env!("CARGO_PKG_VERSION"), " rust edition");

/// The window title. Also folded from the manifest at compile time.
pub const WINDOW_TITLE: &str = concat!("TetraFusion ", env!("CARGO_PKG_VERSION"), " rust edition");

/// The version of the original Pygame game being ported.
///
/// Not the port's own version, which is whatever `Cargo.toml` says. Named, and
/// test-only, because "2.1" is a substring of every 2.2.x version: a test that
/// guards the window title against it has to compare whole versions rather
/// than search the string, and this is where the correct value lives so the
/// comparison cannot drift back into a substring search. See
/// `the_version_shown_in_game_comes_from_the_manifest`.
#[cfg(test)]
pub const ORIGINAL_GAME_VERSION: &str = "2.1";

/// Lines that must be cleared to win a Sprint.
pub const SPRINT_TARGET_LINES: i32 = 40;
/// Duration of an Ultra run, in milliseconds.
pub const ULTRA_DURATION_MS: u64 = 3 * 60 * 1000;

/// How long the level-up colour flash lasts, in milliseconds (the original's
/// `TRANSITION_DURATION`).
pub const LEVEL_TRANSITION_MS: u64 = 2000;
/// How often the stack re-randomises during the level-up flash, in
/// milliseconds (the original's `FLASH_INTERVAL`).
pub const LEVEL_FLASH_INTERVAL: u64 = 100;

/// How long the "TetraFusion!" banner lights the side panel after a four-line
/// clear, in milliseconds (the original's `tetris_flash_time`).
pub const TETRIS_FLASH_MS: u64 = 2000;

/// How long the new-record celebration throws pieces across the screen, in
/// milliseconds.
///
/// Three seconds, and the length is the point rather than a detail. The player
/// is sent to the initials screen the instant a run is recognised as a record,
/// so a celebration that is over in a second is a flash the player spends the
/// whole time typing through, and one that runs much past this stops being an
/// event and becomes the wallpaper. Deliberately longer than
/// [`LEVEL_TRANSITION_MS`] and [`TETRIS_FLASH_MS`]: beating the record outranks
/// clearing four rows, and it is the one moment in a run with no sting of its
/// own to mark it.
pub const HIGH_SCORE_CELEBRATION_MS: u64 = 3000;

/// Default DAS (delayed auto shift) in milliseconds.
pub const DEFAULT_DAS_MS: u32 = 150;
/// Default ARR (auto repeat rate) in milliseconds.
pub const DEFAULT_ARR_MS: u32 = 50;

/// Base block palette, indexed by piece. Index 0 is unused at runtime so
/// that a grid cell of 0 means "empty"; see [`crate::board::Grid`].
pub const COLORS: [[u8; 3]; 8] = [
    [0, 0, 0],       // index 0 is a sentinel; grid cells of 0 mean "empty"
    [0, 230, 230],   // 1 I - cyan
    [255, 150, 0],   // 2 J - orange
    [30, 70, 255],   // 3 L - blue
    [230, 25, 25],   // 4 O - red
    [0, 200, 50],    // 5 S - green
    [245, 245, 60],  // 6 T - yellow
    [190, 60, 220],  // 7 Z - purple
];

/// One colour scheme from the original (`THEMES` in TetraFusion_2.1.py).
/// `colors` are the seven piece colours in piece order; `grid` is the faint
/// lattice colour behind the playfield.
pub struct Theme {
    pub name: &'static str,
    pub colors: [[u8; 3]; 7],
    pub grid: [u8; 3],
}

/// The original's seven themes, verbatim.
pub const THEMES: [Theme; 7] = [
    Theme { name: "Default", colors: [[0, 230, 230], [255, 150, 0], [30, 70, 255], [230, 25, 25], [0, 200, 50], [235, 215, 0], [170, 30, 180]], grid: [200, 200, 200] },
    Theme { name: "Retro", colors: [[0, 240, 240], [240, 160, 0], [0, 0, 240], [240, 0, 0], [0, 240, 0], [240, 240, 0], [160, 0, 240]], grid: [100, 100, 100] },
    Theme { name: "Dark", colors: [[0, 200, 200], [200, 100, 0], [80, 80, 255], [200, 0, 0], [0, 200, 0], [200, 200, 0], [160, 0, 160]], grid: [50, 50, 50] },
    Theme { name: "Pastel", colors: [[150, 255, 255], [255, 210, 150], [150, 150, 255], [255, 150, 150], [150, 255, 150], [255, 255, 150], [210, 150, 210]], grid: [200, 200, 255] },
    Theme { name: "Protanopia", colors: [[0, 200, 200], [200, 160, 0], [0, 100, 255], [200, 100, 100], [100, 200, 0], [255, 255, 0], [150, 0, 150]], grid: [200, 200, 200] },
    Theme { name: "Deuteranopia", colors: [[0, 180, 180], [200, 150, 0], [0, 80, 200], [200, 80, 80], [80, 200, 0], [230, 230, 0], [130, 0, 130]], grid: [200, 200, 200] },
    Theme { name: "Tritanopia", colors: [[0, 200, 150], [200, 120, 80], [0, 60, 180], [220, 60, 60], [60, 200, 80], [255, 220, 0], [180, 0, 120]], grid: [200, 200, 200] },
];

/// How far the palette turns, in whole colour turns, per level.
///
/// 0.12 turns is about 43 degrees: plainly visible from one level to the next,
/// which is the point, but small enough that the seven colours still read as
/// seven colours instead of as an unrelated new set. The level *also* rotates
/// which palette slot each piece draws from (see
/// [`crate::game::Game::color_for`]), and those two together take 58 levels to
/// come back around - longer than any run, so nothing visibly loops.
pub const STAGE_HUE_STEP: f32 = 0.12;

/// Standard Tetris piece colours: the scheme a player can lock back to when
/// they would rather recognise pieces than watch the stage change.
///
/// Same eight-slot shape as [`COLORS`] - index 0 is still the empty sentinel,
/// because the grid stores 0 for an empty cell and the two palettes have to be
/// interchangeable at that one index.
pub const TRADITIONAL: [[u8; 3]; 8] = [
    [0, 0, 0],       // sentinel
    [0, 255, 255],   // 1 I - cyan
    [0, 0, 255],     // 2 J - blue
    [255, 140, 0],   // 3 L - orange
    [255, 255, 0],   // 4 O - yellow
    [0, 220, 60],    // 5 S - green
    [170, 0, 240],   // 6 T - purple
    [235, 0, 0],     // 7 Z - red
];

/// Adaptive: piece colours morph with the stage.
pub const PALETTE_ADAPTIVE: usize = 0;
/// Traditional: standard Tetris colours, locked.
pub const PALETTE_TRADITIONAL: usize = 1;
/// How many palette schemes there are to cycle through.
pub const PALETTE_MODES: usize = 2;

/// Whether `mode` means "lock the standard colours".
///
/// Wheels rather than comparing, so a hand-edited settings file with a huge
/// number lands somewhere real instead of silently meaning Adaptive.
pub fn is_traditional(mode: usize) -> bool {
    mode % PALETTE_MODES == PALETTE_TRADITIONAL
}

/// The name of a palette scheme, for the options row.
pub fn palette_mode_name(mode: usize) -> &'static str {
    if is_traditional(mode) {
        "Traditional"
    } else {
        "Adaptive"
    }
}

/// Red, green and blue to hue/saturation/value, all in `0.0..=1.0`.
///
/// Greys have no hue; they are given hue 0 rather than a NaN, because the
/// palette code multiplies and adds the hue without checking and a NaN would
/// quietly turn a grey block black.
pub fn rgb_to_hsv(c: [u8; 3]) -> [f32; 3] {
    let (r, g, b) = (
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d <= f32::EPSILON {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    let s = if max <= f32::EPSILON { 0.0 } else { d / max };
    [h, s, max]
}

/// Hue/saturation/value back to red, green and blue.
///
/// Hue wraps (`rem_euclid`), so turning a colour past a full turn keeps going
/// instead of flipping to the opposite end - a palette that flipped hue at the
/// wrap would show every seventh level as an unrelated board.
pub fn hsv_to_rgb(hsv: [f32; 3]) -> [u8; 3] {
    let h = hsv[0].rem_euclid(1.0);
    let s = hsv[1].clamp(0.0, 1.0);
    let v = hsv[2].clamp(0.0, 1.0);
    let sector = (h * 6.0).floor();
    let f = h * 6.0 - sector;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match sector as i32 % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    let q8 = |x: f32| (x * 255.0).round().clamp(0.0, 255.0) as u8;
    [q8(r), q8(g), q8(b)]
}

/// Morph a palette into the lighting of `level`.
///
/// The whole set turns together, so the seven pieces stay seven distinct
/// colours while the board as a whole changes character as the stage does.
/// Saturation and value breathe on their own, slower and out of step with the
/// hue, so the palette does not merely rotate like a colour wheel.
///
/// The empty sentinel at index 0 is passed through untouched: a non-black
/// sentinel would be drawn wherever a grid cell reads 0.
pub fn stage_palette(base: &[[u8; 3]; 8], level: i32) -> [[u8; 3]; 8] {
    let t = (level.max(1) - 1) as f32;
    let hue = (t * STAGE_HUE_STEP) % 1.0;
    // Both oscillations are sines, so they are zero at `t == 0` and level one
    // comes out as the theme exactly. A cosine here looks identical and is not:
    // it is at its maximum on the first level, which brightened every colour by
    // 12% before the player had done anything.
    let sat = 1.0 + 0.18 * (t * 0.37).sin();
    let val = 1.0 + 0.12 * (t * 0.23).sin();
    let mut out = *base;
    for i in 1..8 {
        let hsv = rgb_to_hsv(base[i]);
        out[i] = hsv_to_rgb([
            hsv[0] + hue,
            (hsv[1] * sat).clamp(0.0, 1.0),
            (hsv[2] * val).clamp(0.0, 1.0),
        ]);
    }
    out
}

/// The palette the board is actually drawn with.
///
/// Traditional ignores both the theme and the level and hands back the
/// standard colours untouched - that is the whole promise of the option, and it
/// is why it has to be checked *before* the theme is looked up rather than
/// applied on top of one.
pub fn palette_for_settings(theme: usize, mode: usize, level: i32) -> [[u8; 3]; 8] {
    if is_traditional(mode) {
        TRADITIONAL
    } else {
        stage_palette(&palette_for(theme), level)
    }
}

/// Build an 8-slot palette (index 0 is the empty-cell sentinel) from a theme
/// index. Wheels around out-of-range indices the way the original did.
pub fn palette_for(theme: usize) -> [[u8; 3]; 8] {
    let t = &THEMES[theme % THEMES.len()];
    let mut p = [[0, 0, 0]; 8];
    for i in 0..7 {
        p[i + 1] = t.colors[i];
    }
    p
}

// --- Level-up transition -------------------------------------------------

/// How the stack loses its colour on the way to the next level.
///
/// The original only re-randomised the stack's colour indices, which reads as
/// a flicker on an otherwise unchanged board: the shape is identical before and
/// after, so there is nothing to look at except the hue. These add the half that
/// was missing - the blocks themselves change - while every variant keeps *some*
/// colour lit, so the stack never goes flat grey and the board stays readable at
/// the exact moment a level-up puts a new piece on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelFx {
    /// Colour drains out of each block's interior. The block's own rim stays lit
    /// for the whole transition, so the stack reads as a grid of glowing
    /// outlines. This is the plainest reading of "loses its colour but stays
    /// bright around the edges", and it is the most legible at speed because the
    /// rim is a hard geometric feature rather than a soft fade.
    Rim,
    /// Buried blocks drain first and surface blocks last, so the stack is eaten
    /// from the inside out and ends up hollow. Reads as the stack losing its
    /// substance rather than its paint.
    Depth,
    /// A band of colour loss climbs the stack from the floor. The wave is the
    /// only variant whose drain moves through the board, so it is the one that
    /// still looks like a transition once a player has seen it fifty times.
    Wave,
}

/// How many distinct level effects there are.
pub const LEVEL_FX_COUNT: usize = 3;

/// The effect this level's transition uses.
///
/// Deliberately *not* `level % 3`: that lines each effect up with the skin
/// cadence (`LEVELS_PER_SKIN`, also 4) and the palette hue step, so the same
/// three things would change together and the stage would read as one event
/// rather than three. Stepping by 2 over 3 levels guarantees no two adjacent
/// levels pick the same one.
pub fn stage_fx(level: i32) -> LevelFx {
    const ALL: [LevelFx; LEVEL_FX_COUNT] = [LevelFx::Rim, LevelFx::Depth, LevelFx::Wave];
    // Stepping by 2 modulo 3 visits 0, 2, 1, 0, 2, 1 ... so the order is
    // Rim, Wave, Depth. A negative level still names a real effect rather than
    // indexing backwards off the front of the array.
    ALL[((level.clamp(1, i32::MAX) as usize).saturating_sub(1) * 2) % LEVEL_FX_COUNT]
}

/// Where the transition is, `0..=1` across its whole life.
///
/// A triangle: the stack drains over the first 55% and then *snaps back* over
/// the remainder. The recovery is the whole point - a drain that only ever goes
/// one way leaves the board grey and the player waiting for something to happen,
/// and a drain that recovers instantly is a blink. Ramping back gives the level
/// change an ending, which is what makes it feel like an arrival instead of a
/// stall.
pub fn drain_phase(t: f32) -> f32 {
    const OUT: f32 = 0.55;
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
    if t < OUT {
        t / OUT
    } else {
        1.0 - (t - OUT) / (1.0 - OUT)
    }
}

/// How much of a block's colour survives, as `(interior, rim)` multipliers in
/// `0..=1`.
///
/// * `t` is *raw progress* across the transition, `0..=1`. [`drain_phase`] is
///   applied here, not by the caller, so that every caller can pass the same
///   number - elapsed time over `LEVEL_TRANSITION_MS` - and get the triangle.
/// * `exposure` is `Grid::exposure` - `1.0` for a block on the open surface,
///   `0.0` for one buried in the middle of the stack.
/// * `height` is the block's position within the stack, `0.0` at the floor and
///   `1.0` at its highest row. `Wave` uses it and the others ignore it.
///
/// The rim is never allowed to go fully dark. A block that has lost *all* of
/// its colour on both counts would be invisible, and an invisible block is a
/// hole in the board - the player would have to guess whether there was a cell
/// there, which is the one thing a stack full of blocks cannot afford to be
/// ambiguous about.
pub fn fx_keep(fx: LevelFx, t: f32, exposure: f32, height: f32) -> (f32, f32) {
    let exposure = if exposure.is_finite() {
        exposure.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let height = if height.is_finite() {
        height.clamp(0.0, 1.0)
    } else {
        0.0
    };
    // `drained` is the *amount* of colour gone, `0..=1`, peaking mid-transition.
    // `keep` is what is left, and the two are complements - naming both makes
    // the sign error below impossible to write by accident, which is worth the
    // extra binding: the first version of this function subtracted the phase
    // instead of using it, which made the drain vanish at exactly the moment it
    // was supposed to be strongest and pass every test that sampled a
    // non-peak frame.
    let drained = drain_phase(t);
    let keep = 1.0 - drained;
    let (body, rim) = match fx {
        LevelFx::Rim => (keep, 1.0),
        LevelFx::Depth => {
            // Surface blocks hold on longest: they are what the player reads the
            // board from, so they are the last thing to go.
            let keep = keep * (0.35 + 0.65 * exposure);
            (keep, keep)
        }
        LevelFx::Wave => {
            // The band starts on the floor and climbs. `drained * 1.25 - 0.25` is
            // the band's leading edge in stack-height terms, and the `- 0.25` is
            // its half-width: a block drains as the edge passes over it. The
            // leading edge only reaches `1.0` once the drain is 80% done, so the
            // bottom row is never the first thing to change and the top row is
            // never left behind - the band crosses the whole stack inside the
            // draining leg and is gone before the recovery starts.
            //
            // The sign on `height` is the whole effect. Subtracting
            // `(1.0 - height)` instead would put the same band on the ceiling and
            // send it downward, which looks identical in a still and completely
            // wrong in motion, because the eye expects a wave to fall.
            let front = drained * 1.25 - 0.25;
            let band = (front - height * 0.25).clamp(0.0, 1.0);
            let keep = 1.0 - band;
            (keep, keep)
        }
    };
    // The floor lives here rather than in the renderer, so that the promise
    // `fx_keep` makes is the one the player actually sees. Applying it at the
    // draw call instead would let the two disagree: `Rim` returns a rim of
    // exactly 1.0, and a renderer that derived its own rim from the body would
    // draw that block's edge at 0.55 anyway - the brightest edge in the whole
    // stack rendered as a dim one.
    (body, rim.max(MIN_RIM_KEEP))
}

/// The dimmest a block's rim is ever allowed to get.
pub const MIN_RIM_KEEP: f32 = 0.55;

/// How much colour survives on a *floating* piece - the one falling, plus its
/// ghost and the hold/next previews - as the same `(interior, rim)` pair
/// [`fx_keep`] returns for a block in the stack.
///
/// The falling piece is not a special effect of its own. It is a block in the
/// same transition, so it drains with the same variant the level picked, on the
/// same triangle, and it lands in exactly the outline-only state the stack does.
/// Previously it was hardcoded to `(1.0, 1.0)` at the draw call, which meant the
/// effect only ever appeared on blocks that had already stopped moving: the
/// stack drained and the pieces the player was actually steering stayed fully
/// coloured, so the whole thing read as a background animation.
///
/// The two geometry arguments are the piece's honest position in the well, and
/// they are what make it behave like the stack rather than like a special case:
///
/// * `exposure` is `0.0`. A falling piece has no neighbours, so it is exactly as
///   "buried" as the most enclosed block in the stack. For [`LevelFx::Depth`]
///   that means it drains as hard as anything does - which is right, because a
///   piece hanging in open air with nothing holding it up *should* look more
///   spent than a block resting on a surface.
/// * `height` is `1.0`. A piece sits above the stack, so it is at the very top
///   of the well. For [`LevelFx::Wave`] this puts it ahead of the climbing band,
///   so the wave passes the falling piece last and the player watches the stack
///   drain out from under it.
///
/// [`LevelFx::Rim`] ignores both and returns `(keep, 1.0)` - the interior goes
/// and the edge stays lit, which is the effect that was asked for.
pub fn piece_keep(fx: LevelFx, t: f32) -> (f32, f32) {
    fx_keep(fx, t, 0.0, 1.0)
}

/// Take `keep` of `c`'s colour, dropping the rest toward ash.
///
/// The drained colour is a *darkened grey at the same perceived luminance*, not
/// a darkened version of the hue. That distinction is the whole effect: a
/// desaturated block reads as "the colour left it", which is what a level change
/// should look like, whereas simply scaling the channels down reads as "the
/// block is in shadow" and looks like a lighting bug.
pub fn drain_colour(c: [u8; 3], keep: f32) -> [u8; 3] {
    let k = if keep.is_finite() {
        keep.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let luma = 0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32;
    let ash = (luma * 0.34).min(255.0);
    let f = |v: u8, a: f32| -> u8 { (a + (v as f32 - a) * k).round().clamp(0.0, 255.0) as u8 };
    [f(c[0], ash), f(c[1], ash), f(c[2], ash)]
}

#[cfg(test)]
mod fx_tests {
    use super::*;

    /// The transition is the *only* thing standing between a player and a
    /// feature they will never discover, so the rules it has to obey are pinned
    /// here rather than left to whoever edits it next.
    const FX: [LevelFx; LEVEL_FX_COUNT] = [LevelFx::Rim, LevelFx::Depth, LevelFx::Wave];

    /// The headline promise: "the falling pieces lose some of their color,
    /// while keeping the color bright around the edges". Whatever the level
    /// picks, a block that is fully drained must still be visibly a block.
    #[test]
    fn a_fully_drained_block_still_has_a_lit_edge() {
        for fx in FX {
            for t in [0.0, 0.2, 0.4, 0.55, 0.7, 0.9, 1.0] {
                for exposure in [0.0, 0.5, 1.0] {
                    for height in [0.0, 0.5, 1.0] {
                        let (interior, rim) = fx_keep(fx, t, exposure, height);
                        assert!(
                            rim >= MIN_RIM_KEEP,
                            "{fx:?} t={t} rim fell to {rim}"
                        );
                        assert!(
                            (0.0..=1.0).contains(&interior),
                            "{fx:?} t={t} interior {interior}"
                        );
                        assert!(
                            rim >= interior,
                            "{fx:?} t={t}: the rim {rim} is darker than the body \
                             {interior}, which is the opposite of the effect"
                        );
                    }
                }
            }
        }
    }

    /// The rim has to actually *lead*: it has to hold colour after the body has
    /// given it up, or the two numbers above are equal and the effect is a
    /// plain fade. `Rim` is the variant that promises it outright.
    #[test]
    fn the_rim_outlasts_the_body() {
        for t in [0.1, 0.25, 0.4] {
            let (interior, rim) = fx_keep(LevelFx::Rim, t, 0.0, 0.0);
            assert!(
                rim > interior,
                "at t={t} the rim ({rim}) should still beat the body ({interior})"
            );
            assert_eq!(rim, 1.0, "Rim promises an undrained edge at every t");
        }
    }

    /// The peak of the triangle is a real peak, not a plateau and not a
    /// rounding artefact. Everything below depends on the drain actually
    /// reaching 1, so it is pinned here rather than inferred from the callers.
    #[test]
    fn the_drain_actually_reaches_full_emptiness() {
        assert_eq!(drain_phase(0.0), 0.0, "the drain starts empty-handed");
        assert_eq!(drain_phase(0.55), 1.0, "the peak is 1, at 55% of the window");
        assert!(drain_phase(0.3) > 0.0 && drain_phase(0.3) < 1.0, "mid-drain");
    }

    /// Colour has to actually go. A transition that leaves the stack at full
    /// saturation is not a transition, and "somewhere on the board" is the right
    /// bar: `Depth` saves the surface and `Wave` saves everything above the
    /// band, so neither is uniform, and demanding a uniformly dark stack would
    /// be demanding the effect the player is being denied.
    #[test]
    fn the_stack_really_loses_its_colour_at_the_peak() {
        // Sampled at the peak of the triangle, which is the point in *raw*
        // progress where `drain_phase` is 1.
        let peak = 0.55;
        assert_eq!(drain_phase(peak), 1.0);
        for fx in FX {
            let mut darkest = 1.0f32;
            for e in 0..=10 {
                for h in 0..=10 {
                    let (interior, _) =
                        fx_keep(fx, peak, e as f32 / 10.0, h as f32 / 10.0);
                    darkest = darkest.min(interior);
                }
            }
            assert!(
                darkest < 0.2,
                "{fx:?} never dropped below {darkest} anywhere on the board"
            );
        }
    }

    /// And it has to come back, or the board is grey for the rest of the level
    /// and the player is left waiting for the level change to finish.
    #[test]
    fn the_colour_comes_back_before_the_transition_ends() {
        assert_eq!(drain_phase(0.0), 0.0, "no drain at the start");
        assert_eq!(drain_phase(1.0), 0.0, "fully recovered at the end");
        assert!(drain_phase(0.99) < 0.05, "still draining at 99%");
        // And the recovery is monotonic, so the stack cannot flicker back and
        // forth on the way out.
        let mut prev = drain_phase(0.6);
        for i in 1..=40 {
            let t = 0.6 + (i as f32) * 0.01;
            let now = drain_phase(t);
            assert!(now <= prev + 1e-6, "the drain came back up at t={t}");
            prev = now;
        }
    }

    /// Every level gets an effect, and no two in a row are the same. Adjacent
    /// levels sharing an effect is the difference between "each stage has its
    /// own look" and "an effect that happens sometimes".
    #[test]
    fn every_level_names_an_effect_and_neighbours_differ() {
        let mut seen = Vec::new();
        for level in 1..=64 {
            let fx = stage_fx(level);
            assert!(FX.contains(&fx), "level {level} named {fx:?}");
            if let Some(prev) = seen.last() {
                assert_ne!(prev, &fx, "levels {level} and {} are both {fx:?}", level - 1);
            }
            seen.push(fx);
        }
        // And over a long run every effect is actually used.
        for fx in FX {
            assert!(
                (1..=64).any(|l| stage_fx(l) == fx),
                "{fx:?} never came up in 64 levels"
            );
        }
    }

    /// A level outside the playable range must still name a real effect rather
    /// than indexing backwards off the front of the array. `clamp(.., 1, ..)`
    /// is what does that, and it is load-bearing for a level-0 debug path.
    #[test]
    fn a_level_outside_the_scale_still_names_a_real_effect() {
        for level in [i32::MIN, -500, -1, 0] {
            assert!(
                FX.contains(&stage_fx(level)),
                "level {level} named something outside the enum"
            );
        }
        // And all four agree, because a level below 1 is not a *different*
        // level, it is the first level.
        assert_eq!(stage_fx(0), stage_fx(1));
    }

    /// The three effects must be *different effects*, or the choice is a lie.
    ///
    /// Sampled mid-drain, because at `t = 0` and `t = 1` all three collapse to
    /// the same value by construction and a comparison there proves nothing.
    #[test]
    fn the_three_effects_are_measurably_different() {
        // Mid-drain by another name: a raw-progress point whose `drain_phase`
        // is comfortably inside the triangle. At `t = 0` and `t = 1` all three
        // collapse to the same value by construction, so a comparison there
        // proves nothing about whether they differ.
        let mid = 0.3;
        let phase = drain_phase(mid);
        assert!(phase > 0.0 && phase < 1.0, "sample point should be mid-drain");

        // `Rim` ignores both inputs, so it is the control: it must be flat
        // across the board, and it must be that flat.
        let rim_spread = (fx_keep(LevelFx::Rim, mid, 0.0, 0.0).0
            - fx_keep(LevelFx::Rim, mid, 1.0, 1.0).0)
            .abs();
        assert!(rim_spread < 1e-6, "Rim should not vary across the board");

        let depth_spread = (fx_keep(LevelFx::Depth, mid, 0.0, 0.5).0
            - fx_keep(LevelFx::Depth, mid, 1.0, 0.5).0)
            .abs();
        assert!(
            depth_spread > 0.2,
            "Depth should separate buried from surface blocks, was {depth_spread}"
        );

        let wave_spread =
            (fx_keep(LevelFx::Wave, mid, 0.5, 0.0).0 - fx_keep(LevelFx::Wave, mid, 0.5, 1.0).0)
                .abs();
        assert!(
            wave_spread > 0.2,
            "Wave should separate the floor from the top, was {wave_spread}"
        );

        // Depth keys off exposure and Wave off height, so each must be *blind*
        // to the other's input. If they were not, the two would collapse into
        // one effect and the cycle would only really have two.
        assert!(
            (fx_keep(LevelFx::Depth, mid, 0.5, 0.0).0
                - fx_keep(LevelFx::Depth, mid, 0.5, 1.0).0)
                .abs()
                < 1e-6,
            "Depth should ignore height"
        );
        assert!(
            (fx_keep(LevelFx::Wave, mid, 0.0, 0.5).0 - fx_keep(LevelFx::Wave, mid, 1.0, 0.5).0)
                .abs()
                < 1e-6,
            "Wave should ignore exposure"
        );
    }

    /// `Depth` has to hold the *surface* longer than the interior. If it ran
    /// the other way the effect would hollow out the visible surface first,
    /// which is the opposite of keeping the board readable.
    #[test]
    fn depth_holds_the_surface_longest() {
        let t = drain_phase(0.2);
        let buried = fx_keep(LevelFx::Depth, t, 0.0, 0.5).0;
        let surface = fx_keep(LevelFx::Depth, t, 1.0, 0.5).0;
        assert!(
            surface > buried,
            "surface {surface} should still beat buried {buried}"
        );
    }

    /// The wave has to move *upward* through the stack over time. A static
    /// gradient would look banded with nothing to explain it; a wave that ran
    /// the other way would look like the stack was melting, which is the wrong
    /// story for a level the player just cleared.
    ///
    /// The signature of a climbing wave is that the *lowest* row is the first to
    /// start losing colour. If the sign on `height` is ever flipped this is the
    /// test that catches it.
    #[test]
    fn the_wave_climbs_the_stack() {
        // Sampled early, while the band is still in the lower half. A block low
        // in the stack drains before one high up, so the low block's keep value
        // is already lower than the high block's.
        let t = drain_phase(0.2);
        let low = fx_keep(LevelFx::Wave, t, 0.5, 0.0).0;
        let high = fx_keep(LevelFx::Wave, t, 0.5, 1.0).0;
        assert!(
            low < high,
            "the wave should reach the floor first: floor {low} vs top {high}"
        );

        // And the ordering must hold at *every* point while the band is on the
        // board, not just the one sample above. Once the band has fully crossed
        // the stack the difference legitimately goes to zero.
        for i in 1..=40 {
            let t = drain_phase(i as f32 / 50.0);
            let low = fx_keep(LevelFx::Wave, t, 0.5, 0.0).0;
            let high = fx_keep(LevelFx::Wave, t, 0.5, 1.0).0;
            assert!(
                low <= high + 1e-6,
                "at t={t} the floor ({low}) was lighter than the top ({high})"
            );
        }
    }

    /// The bug this whole helper exists for.
    ///
    /// `draw_piece` used to hardcode `(1.0, 1.0)` for the live piece, its ghost
    /// and the hold/next previews, so the drain only ever appeared on blocks that
    /// had already stopped moving. The stack went grey and the piece the player
    /// was steering stayed fully coloured, and the effect read as a background
    /// animation rather than something happening to the game.
    ///
    /// It is easy to reintroduce: the piece is not in the grid, so `exposure` and
    /// `height` do not obviously apply to it, and passing `(1.0, 1.0)` looks
    /// like the safe default. This asserts the piece genuinely loses colour at
    /// the peak of the transition, for every variant.
    #[test]
    fn the_falling_piece_drains_too() {
        // The peak of the triangle, where the drain is strongest.
        let peak = 0.5;
        for fx in [LevelFx::Rim, LevelFx::Depth, LevelFx::Wave] {
            let (rest_body, _) = piece_keep(fx, 0.0);
            let (peak_body, _) = piece_keep(fx, peak);
            assert!(
                peak_body < rest_body,
                "{fx:?}: the falling piece never lost colour \
                 (start {rest_body}, peak {peak_body})"
            );
        }
    }

    /// The look that was actually asked for: the colour leaves the inside of the
    /// piece and the edge stays lit. `Rim` is the variant that does this, and it
    /// is the one that has to hold at *every* frame - not just at the peak - or
    /// the outline fades out along with the fill and the piece just looks dim.
    #[test]
    fn the_piece_keeps_its_edge_while_its_inside_goes() {
        for i in 0..=50 {
            let t = i as f32 / 50.0;
            let (body, rim) = piece_keep(LevelFx::Rim, t);
            assert!(
                rim >= 1.0 - 1e-6,
                "{fx:?} at t={t}: the piece's edge dimmed to {rim}",
                fx = LevelFx::Rim
            );
            assert!(
                body <= rim,
                "{fx:?} at t={t}: the inside ({body}) outranked the edge ({rim})",
                fx = LevelFx::Rim
            );
        }
        // And it really does lose its inside at the peak, not just theoretically.
        let (body, rim) = piece_keep(LevelFx::Rim, 0.5);
        assert!(body < 0.2, "the piece's inside barely drained: {body}");
        assert!((rim - 1.0).abs() < 1e-6, "the edge should be untouched");
    }

    /// A piece hanging in open air with nothing holding it up should look at
    /// least as spent as the most enclosed block in the stack. If this ever
    /// flips, the piece has been given a privileged exposure and the depth
    /// variant stops reading as depth.
    #[test]
    fn a_floating_piece_drains_as_hard_as_the_most_buried_block() {
        let t = 0.5;
        let piece = piece_keep(LevelFx::Depth, t).0;
        let buried = fx_keep(LevelFx::Depth, t, 0.0, 0.5).0;
        assert!(
            piece <= buried + 1e-6,
            "the falling piece ({piece}) kept more colour than a buried block \
             ({buried})"
        );
    }

    /// The piece must not be special-cased anywhere: same variant, same triangle,
    /// same peak, same recovery. It is a block in the same transition.
    #[test]
    fn the_piece_shares_the_stack_clock_exactly() {
        for fx in [LevelFx::Rim, LevelFx::Depth, LevelFx::Wave] {
            for i in 0..=50 {
                let t = i as f32 / 50.0;
                let piece = piece_keep(fx, t);
                // A piece is at the top of the well with no neighbours.
                let block = fx_keep(fx, t, 0.0, 1.0);
                assert_eq!(piece, block, "{fx:?} disagreed at t={t}");
            }
        }
    }

    /// The drain ramps out and comes back, and the piece has to do both. A piece
    /// that only drained would end every level change stuck grey.
    ///
    /// Note what "recovers" does *not* mean here: not full colour. `Depth` weights
    /// every block by `0.35 + 0.65 * exposure`, so a floating piece's value with
    /// no drain left at all is `0.35`, and `Wave`'s is `1.0`. That is the variant
    /// working as designed rather than the piece being stuck - nothing is
    /// permanently dimmed, because outside a transition `fx` is `None` and every
    /// block is drawn at `(1.0, 1.0)`. The claim under test is that the piece
    /// brightens again by the end, not that it reaches any particular value.
    #[test]
    fn the_piece_recovers_at_the_end_of_the_transition() {
        for fx in [LevelFx::Rim, LevelFx::Depth, LevelFx::Wave] {
            let (peak, _) = piece_keep(fx, 0.5);
            let (end, _) = piece_keep(fx, 1.0);
            assert!(
                end > peak,
                "{fx:?}: the piece never recovered (peak {peak}, end {end})"
            );
        }
        // `Rim` and `Wave` do land on full colour, and it is worth pinning that
        // they do: it is the check that the triangle returns all the way home.
        for fx in [LevelFx::Rim, LevelFx::Wave] {
            let (end, _) = piece_keep(fx, 1.0);
            assert!(
                (end - 1.0).abs() < 1e-3,
                "{fx:?}: expected full colour at the end, got {end}"
            );
        }
        // `Depth`'s resting value is its exposure floor, and a piece in open air
        // sits at that floor because it has no neighbours to hold colour onto.
        let (depth_end, _) = piece_keep(LevelFx::Depth, 1.0);
        assert!(
            (depth_end - 0.35).abs() < 1e-3,
            "Depth should rest at its exposure floor of 0.35, got {depth_end}"
        );
    }

    /// Nothing here may produce a colour multiplier the renderer would choke on.
    /// The rim floor is `fx_keep`'s promise, and the piece inherits it.
    #[test]
    fn the_piece_never_goes_darker_than_the_rim_floor() {
        for fx in [LevelFx::Rim, LevelFx::Depth, LevelFx::Wave] {
            for i in 0..=100 {
                let t = i as f32 / 100.0;
                let (body, rim) = piece_keep(fx, t);
                assert!(
                    (0.0..=1.0).contains(&body),
                    "{fx:?} at t={t}: interior {body} outside 0..=1"
                );
                assert!(
                    rim >= MIN_RIM_KEEP - 1e-6 && rim <= 1.0 + 1e-6,
                    "{fx:?} at t={t}: rim {rim} outside \
                     {MIN_RIM_KEEP}..=1"
                );
            }
        }
    }

    /// The band has to be gone before the drain starts recovering, or the
    /// recovery races the wave and the two effects fight each other.
    #[test]
    fn the_wave_finishes_before_the_drain_recovers() {
        // Raw progress 0.4 is deep into the draining leg; 0.75 is well into the
        // recovery. Both are past the point where the band should have crossed
        // the floor, so the floor must be climbing back toward full colour.
        let at_peak = drain_phase(0.4);
        let after = drain_phase(0.75);
        assert!(after < at_peak, "the sample points must straddle the peak");
        let floor_after = fx_keep(LevelFx::Wave, 0.75, 0.5, 0.0).0;
        assert!(
            floor_after > 0.5,
            "the floor should have recovered to {floor_after}"
        );
    }

    /// Every `keep` value a caller could realistically produce has to produce a
    /// real colour. `keep` arrives from `fx_keep` divided by a `rows` span, so
    /// out-of-range and non-finite inputs are reachable in principle and a
    /// wrapping `as u8` cast turns them into an unrelated colour - a block that
    /// flickers between two hues the palette has never contained. The check is
    /// a result comparison rather than a bounds test, because a `u8` is
    /// structurally incapable of exceeding 255 and a bounds test on one can only
    /// ever pass.
    #[test]
    fn draining_a_colour_always_lands_in_the_palette() {
        for c in [[0u8, 0, 0], [255, 255, 255], [255, 0, 0], [0, 0, 255], [3, 251, 7]] {
            for keep in [-1.0, 0.0, 0.5, 1.0, 2.0] {
                let out = drain_colour(c, keep);
                let ash = (0.299 * c[0] as f32 + 0.587 * c[1] as f32
                    + 0.114 * c[2] as f32)
                    * 0.34;
                for (v, src) in out.iter().zip(c) {
                    let want = (ash + (src as f32 - ash) * keep.clamp(0.0, 1.0))
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    assert_eq!(*v, want, "{c:?} at keep={keep} gave {out:?}");
                }
            }
        }
    }

    /// A non-finite `keep` must be treated as *no* colour rather than as full
    /// colour. The asymmetry is deliberate and is the safe direction to fail in:
    /// a NaN arriving from a divide-by-zero span should not paint every block in
    /// the stack at full saturation, because that is indistinguishable from the
    /// drain not running at all.
    #[test]
    fn a_broken_keep_drains_rather_than_saturating() {
        let drained = drain_colour([255, 40, 10], 0.0);
        for bad in [f32::NAN, f32::INFINITY, -f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                drain_colour([255, 40, 10], bad),
                drained,
                "{bad} should be treated as no colour"
            );
        }
    }

    /// Fully drained means *grey*, not dark. Scaling the channels down instead
    /// would read as a shadow, and the whole point is that the colour left.
    #[test]
    fn a_fully_drained_block_is_grey_rather_than_black() {
        let out = drain_colour([255, 40, 10], 0.0);
        let spread = out.iter().map(|&v| v as i32);
        let lo = spread.clone().min().unwrap();
        let hi = spread.max().unwrap();
        assert_eq!(lo, hi, "a drained block should be grey, got {out:?}");
        assert!(hi > 0, "a drained block should not go black: {out:?}");
        assert!(hi < 255, "a drained block should not stay bright: {out:?}");
    }

    /// At full colour the drain is the identity, or every block in the game
    /// would be drawn slightly wrong on every frame that happens to be normal.
    #[test]
    fn a_fully_kept_block_is_exactly_its_colour() {
        for c in [[0u8, 0, 0], [255, 40, 10], [7, 200, 99]] {
            assert_eq!(drain_colour(c, 1.0), c);
        }
    }

    /// `keep` has to be monotonic: more colour kept is never a darker result.
    /// A non-monotonic ramp would make the stack pulse as the transition ran.
    #[test]
    fn keeping_more_colour_is_never_darker() {
        let c = [180, 90, 40];
        let mut prev = [0u8; 3];
        for i in 0..=20 {
            let out = drain_colour(c, i as f32 / 20.0);
            assert!(
                out[0] >= prev[0] && out[1] >= prev[1] && out[2] >= prev[2],
                "keep={} produced {out:?}, darker than the previous {prev:?}",
                i as f32 / 20.0
            );
            prev = out;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The floor and the gravity ceiling are the same number said two ways.
    ///
    /// `MIN_FALL_SPEED` (50 ms/row) is twenty rows per second; `MAX_GRAVITY`
    /// (20 G) is twenty rows per second. The whole design calls the top of the
    /// range "20G", so if these ever drift apart the name stops describing the
    /// game.
    #[test]
    fn the_gravity_floor_is_exactly_the_maximum_gravity() {
        assert_eq!(MIN_FALL_SPEED * MAX_GRAVITY, 1000);
    }

    #[test]
    fn gravity_is_one_g_at_level_one_and_rises_to_the_ceiling() {
        assert_eq!(gravity_g(1), 1);
        assert_eq!(gravity_g(2), 2);
        assert_eq!(gravity_g(10), 10);
        assert_eq!(gravity_g(20), MAX_GRAVITY);
    }

    /// Past the ceiling, and below the floor of the scale, gravity clamps
    /// rather than running away or dividing by zero.
    #[test]
    fn gravity_clamps_outside_the_scale_at_both_ends() {
        for level in [21, 25, 100, i32::MAX] {
            assert_eq!(gravity_g(level), MAX_GRAVITY, "level {level}");
        }
        for level in [0, -1, -1000, i32::MIN] {
            assert_eq!(gravity_g(level), 1, "level {level}");
        }
    }

    /// The ramp never stalls or reverses: every level pulls at least as hard as
    /// the one below it, which is the whole point of the redesign.
    #[test]
    fn gravity_never_slows_down_as_the_level_climbs() {
        let mut prev = 0u32;
        for level in 1..=40 {
            let g = gravity_g(level);
            assert!(g >= prev, "level {level} is slower than the level below");
            assert!(g >= 1, "level {level} froze the piece");
            prev = g;
        }
    }

    /// On normal, 20G lands exactly on level 20 - the level the design names.
    #[test]
    fn normal_difficulty_reaches_twenty_g_at_level_twenty() {
        let base = 1000u32;
        assert_eq!(fall_speed_for(base, 1), 1000);
        assert_eq!(fall_speed_for(base, 10), 100);
        assert_eq!(fall_speed_for(base, 20), MIN_FALL_SPEED);
    }

    /// No difficulty may ever exceed the ceiling, however fast its base.
    #[test]
    fn no_difficulty_can_fall_faster_than_twenty_g() {
        for base in [1u32, 10, 50, 100, 200, 400, 600, 1000, 1500, 60_000] {
            for level in 1..=60 {
                let ms = fall_speed_for(base, level);
                assert!(
                    ms >= MIN_FALL_SPEED,
                    "base {base} at level {level} gave {ms} ms/row, past 20G"
                );
            }
        }
    }

    /// A cadence below one row per second is still a real time, not zero.
    #[test]
    fn a_very_slow_difficulty_does_not_round_away_to_nothing() {
        // 1000 / 20 is exactly 50, but a base that is not a multiple of the
        // gravity has to round to something at least one millisecond wide.
        for level in 1..=20 {
            let ms = fall_speed_for(999, level);
            assert!(ms >= 1, "level {level} produced a zero-width interval");
        }
    }

    /// The curve is monotonic per difficulty, never picking up speed again
    /// after the ceiling has flattened it out.
    #[test]
    fn each_difficulty_speeds_up_monotonically_and_then_holds() {
        for base in [1500u32, 1000, 600, 400, 200] {
            let mut prev = u32::MAX;
            for level in 1..=40 {
                let ms = fall_speed_for(base, level);
                assert!(ms <= prev, "base {base} level {level} slowed down");
                prev = ms;
            }
            assert_eq!(
                fall_speed_for(base, 40),
                fall_speed_for(base, 20),
                "base {base} should be flat past level 20"
            );
        }
    }

    // --- colour maths ----------------------------------------------------

    /// Converting to HSV and back has to be lossless, because the stage palette
    /// is built entirely out of that round trip. A lossy one would quietly
    /// shift every colour by a channel or two on the way to the screen.
    #[test]
    fn hsv_round_trips_through_rgb() {
        for c in COLORS.iter().chain(TRADITIONAL.iter()) {
            let back = hsv_to_rgb(rgb_to_hsv(*c));
            assert_eq!(back, *c, "{c:?} came back as {back:?}");
        }
        for c in [
            [0u8, 0, 0],
            [255, 255, 255],
            [1, 2, 3],
            [200, 200, 200],
            [12, 200, 37],
        ] {
            let back = hsv_to_rgb(rgb_to_hsv(c));
            assert_eq!(back, c, "{c:?} came back as {back:?}");
        }
    }

    /// A hue of any size wraps rather than exploding: the stage palette turns
    /// past a full turn on long runs, and wrapping is what keeps that from
    /// producing out-of-range channels.
    #[test]
    fn hue_wraps_instead_of_running_away() {
        for turns in [-3.5f32, -1.0, 0.0, 1.0, 2.5, 100.25] {
            let c = hsv_to_rgb([turns, 1.0, 1.0]);
            let again = hsv_to_rgb([turns.rem_euclid(1.0), 1.0, 1.0]);
            assert_eq!(c, again, "hue {turns} did not wrap");
        }
        assert_eq!(hsv_to_rgb([0.0, 1.0, 1.0]), [255, 0, 0]);
    }

    /// A grey has no hue, and must come back as the same grey rather than as a
    /// NaN that quietly paints the block black.
    #[test]
    fn a_grey_has_a_usable_hue_instead_of_a_nan() {
        let grey = [128u8, 128, 128];
        let hsv = rgb_to_hsv(grey);
        assert!(hsv[0].is_finite(), "grey hue was not finite");
        assert_eq!(hsv[1], 0.0, "grey should have no saturation");
        assert_eq!(hsv_to_rgb(hsv), grey);
        assert_eq!(rgb_to_hsv([0, 0, 0])[1], 0.0);
    }

    // --- the stage palette ----------------------------------------------

    /// The empty sentinel must stay empty. A grid cell of 0 means "no block",
    /// and anything that coloured it would put a stray block on the board.
    #[test]
    fn the_stage_palette_leaves_the_empty_sentinel_alone() {
        let base = palette_for(0);
        for level in 1..=60 {
            assert_eq!(stage_palette(&base, level)[0], base[0], "level {level}");
            assert_eq!(
                stage_palette(&base, level)[0],
                [0, 0, 0],
                "level {level} painted the sentinel"
            );
        }
    }

    /// The whole point: the palette has to visibly change from one level to the
    /// next, or a run looks identical from line one to line two hundred.
    #[test]
    fn each_level_visibly_changes_the_palette() {
        let base = palette_for(0);
        for level in 2..=40 {
            let a = stage_palette(&base, level - 1);
            let b = stage_palette(&base, level);
            let moved = (1..8)
                .map(|i| {
                    (0..3)
                        .map(|c| (a[i][c] as i32 - b[i][c] as i32).abs())
                        .max()
                        .unwrap()
                })
                .max()
                .unwrap();
            assert!(
                moved > 8,
                "level {level} only moved the palette by {moved}"
            );
        }
    }

    /// Level one is the theme exactly as the original drew it - no morph, no
    /// drift - so a fresh run opens on the colours the player recognises.
    #[test]
    fn level_one_is_the_theme_untouched() {
        let base = palette_for(0);
        assert_eq!(stage_palette(&base, 1), base);
    }

    /// The version the game announces comes from `Cargo.toml`, not from a
    /// typed string.
    ///
    /// This exists because it did not. `Cargo.toml` was bumped to 2.2.0 while
    /// the window title and the main menu subtitle still said 2.1, both of them
    /// string literals nobody remembered to edit. Reading the manifest at
    /// compile time makes that class of drift impossible, and this test fails
    /// if a future version bump reintroduces a hardcoded number.
    #[test]
    fn the_version_shown_in_game_comes_from_the_manifest() {
        // Read Cargo.toml the way the compiler does, so the assertion is
        // against the real file rather than a copy of it.
        let manifest = include_str!("../Cargo.toml");
        // `version = "x.y.z"` in the `[package]` table. The value is found by
        // parsing rather than by a hardcoded string so this test keeps
        // working after the next bump.
        let declared = manifest
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                let rest = line.strip_prefix("version")?.trim_start();
                let rest = rest.strip_prefix('=')?.trim();
                Some(rest.trim_matches('"').to_string())
            })
            .next()
            .expect("Cargo.toml should declare a package version");

        assert_eq!(
            VERSION_LINE,
            format!("{declared} rust edition"),
            "the main menu subtitle should be the manifest version"
        );
        assert_eq!(
            WINDOW_TITLE,
            format!("TetraFusion {declared} rust edition"),
            "the window title should be the manifest version"
        );
        // The original Pygame game is 2.1, and the window title must announce
        // this port's own version rather than the one being ported.
        //
        // Compared as whole versions, not as a substring. `contains("2.1")` was
        // the wrong test and it broke on 2.2.1: every 2.2.x version has "2.1" in
        // it, so the guard fired on the port announcing its own version
        // correctly. That is the failure mode substring checks have, and it only
        // shows up on a version bump.
        assert_ne!(
            declared, ORIGINAL_GAME_VERSION,
            "the window title announces {ORIGINAL_GAME_VERSION}, which is the version \
             being ported, not this port's own"
        );
        assert_eq!(
            WINDOW_TITLE,
            format!("TetraFusion {declared} rust edition"),
            "the window title should carry this port's version and not the original's"
        );
    }

    /// The morph must not wash a colour out or crush it to black.
    ///
    /// The test is on saturation, not on the brightest channel. A Pastel theme
    /// already sits at 255 in two channels, so any brightening clips *a*
    /// channel - that is not the colour going white, and demanding no channel
    /// ever reaches 255 would fail every theme that starts out bright while
    /// catching nothing. What matters is that the colour is still a colour.
    #[test]
    fn the_stage_palette_never_washes_a_piece_out() {
        for theme in 0..THEMES.len() {
            let base = palette_for(theme);
            for level in 1..=80 {
                let p = stage_palette(&base, level);
                for i in 1..8 {
                    let hsv = rgb_to_hsv(p[i]);
                    assert!(
                        hsv[2] > 0.15,
                        "theme {theme} level {level} piece {i} went near-black: {:?}",
                        p[i]
                    );
                    assert!(
                        hsv[1] > 0.20,
                        "theme {theme} level {level} piece {i} washed out to {:?}",
                        p[i]
                    );
                }
            }
        }
    }

    /// The seven pieces have to stay seven different colours through the morph,
    /// or the palette stops identifying pieces at all.
    #[test]
    fn the_morphed_palette_keeps_seven_distinguishable_colours() {
        let base = palette_for(0);
        for level in 1..=80 {
            let p = stage_palette(&base, level);
            for i in 1..8 {
                for j in i + 1..8 {
                    let hi = rgb_to_hsv(p[i])[0];
                    let hj = rgb_to_hsv(p[j])[0];
                    let d = (hi - hj).abs().min(1.0 - (hi - hj).abs());
                    assert!(
                        d > 0.02,
                        "level {level} merged pieces {i} and {j} at the same hue"
                    );
                }
            }
        }
    }

    /// Traditional is the escape hatch, and it has to be absolute: the same
    /// standard colours at every level, whatever theme or level is set.
    #[test]
    fn traditional_ignores_both_the_theme_and_the_level() {
        for theme in 0..THEMES.len() {
            for level in [1, 2, 7, 20, 99] {
                assert_eq!(
                    palette_for_settings(theme, PALETTE_TRADITIONAL, level),
                    TRADITIONAL,
                    "theme {theme} level {level}"
                );
            }
        }
        assert!(is_traditional(PALETTE_TRADITIONAL));
        assert!(!is_traditional(PALETTE_ADAPTIVE));
        assert_eq!(palette_mode_name(PALETTE_TRADITIONAL), "Traditional");
        assert_eq!(palette_mode_name(PALETTE_ADAPTIVE), "Adaptive");
    }

    /// An out-of-range mode from a hand-edited file has to land somewhere real.
    #[test]
    fn an_out_of_range_palette_mode_wheels_like_the_theme_list() {
        // Even numbers land on Adaptive, odd on Traditional, however large.
        for mode in 0..PALETTE_MODES * 3 {
            assert_eq!(
                is_traditional(mode),
                mode % PALETTE_MODES == PALETTE_TRADITIONAL,
                "mode {mode}"
            );
        }
        assert!(is_traditional(PALETTE_MODES + PALETTE_TRADITIONAL));
        assert!(!is_traditional(PALETTE_MODES + PALETTE_ADAPTIVE));
        assert_eq!(
            palette_mode_name(usize::MAX),
            palette_mode_name(usize::MAX % PALETTE_MODES)
        );
    }

    /// A crude hue name, so the Traditional palette can be checked against what
    /// a player actually recognises rather than against the numbers.
    ///
    /// The bin edges sit halfway between the six primaries plus a split for
    /// orange, on the standard wheel: 0 red, 1/6 yellow, 2/6 green, 3/6 cyan,
    /// 4/6 blue, 5/6 magenta. An earlier set of edges put cyan in "green" and
    /// purple in "blue", which is exactly the kind of error this exists to
    /// catch and would have shipped a palette claiming to be standard while
    /// failing the one test written about it.
    fn hue_name(c: [u8; 3]) -> &'static str {
        let hsv = rgb_to_hsv(c);
        let h = hsv[0];
        if hsv[1] < 0.15 {
            "grey"
        } else if h < 0.045 || h > 0.955 {
            "red"
        } else if h < 0.125 {
            "orange"
        } else if h < 0.208 {
            "yellow"
        } else if h < 0.458 {
            "green"
        } else if h < 0.542 {
            "cyan"
        } else if h < 0.72 {
            "blue"
        } else {
            "purple"
        }
    }

    /// The Traditional colours really are standard Tetris: I cyan, O yellow,
    /// J blue and L orange. Getting J and L the wrong way round is the classic
    /// way to ship a "standard" palette that is not.
    #[test]
    fn the_traditional_palette_is_standard_tetris() {
        assert_eq!(hue_name(TRADITIONAL[1]), "cyan", "I should be cyan");
        assert_eq!(hue_name(TRADITIONAL[2]), "blue", "J should be blue");
        assert_eq!(hue_name(TRADITIONAL[3]), "orange", "L should be orange");
        assert_eq!(hue_name(TRADITIONAL[4]), "yellow", "O should be yellow");
        assert_eq!(hue_name(TRADITIONAL[5]), "green", "S should be green");
        assert_eq!(hue_name(TRADITIONAL[6]), "purple", "T should be purple");
        assert_eq!(hue_name(TRADITIONAL[7]), "red", "Z should be red");
        assert_eq!(TRADITIONAL[0], [0, 0, 0], "index 0 is the empty sentinel");
    }

    /// Adaptive is the default, so a fresh install morphs rather than sitting
    /// still - and Traditional is something the player opts into.
    #[test]
    fn adaptive_is_what_a_fresh_game_gets() {
        let base = palette_for(0);
        assert_eq!(
            palette_for_settings(0, PALETTE_ADAPTIVE, 1),
            base,
            "level one should look like the theme"
        );
        assert_ne!(
            palette_for_settings(0, PALETTE_ADAPTIVE, 5),
            base,
            "level five should have moved on"
        );
    }
}
