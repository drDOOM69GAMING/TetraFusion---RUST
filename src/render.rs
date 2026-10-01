//! All drawing. Kept separate from [`crate::game`] so the rules stay testable
//! without a window.
//!
//! Every function takes a `&mut RaylibDrawHandle`, the per-frame drawing
//! context raylib-rs 5.x hands out.

use rand::Rng;
use raylib::core::color::Color;
use raylib::core::drawing::{RaylibDraw, RaylibDrawHandle};
use raylib::core::texture::RenderTexture2D;
use raylib::ffi::{Rectangle, Vector2};

use crate::board::TOTAL_ROWS;
use crate::config::*;
use crate::game::{Event, Game, Mode};
use crate::pieces::{self, Piece, Rotation};

fn rgb(c: [u8; 3]) -> Color {
    Color::new(c[0], c[1], c[2], 255)
}

/// The original's per-level colour drift.
///
/// Two separate things change colour as the level climbs, and they are easy to
/// confuse:
///
/// * [`crate::game::Game::color`] rotates the *palette index*, so an I piece is
///   cyan on level 1 and orange on level 2. That is the change a player notices.
/// * this adds a small per-channel drift on top, so even a settled block that
///   keeps its index slowly warms or cools.
///
/// The drift is three sinusoids a third of a cycle apart with amplitudes of 12,
/// 8 and 10 out of 255 - a few percent - so it reads as the palette breathing
/// rather than as a different set of colours. `t = level * 0.25` means the whole
/// cycle takes 25 levels to come back around, which is slower than a player
/// reaches in one sitting and so never reads as a loop.
///
/// Two details are load-bearing:
///
/// * The phases are the literal `2.094` and `4.189` from the original, not
///   `2*PI/3` and `4*PI/3`. They are rounded versions of those, and over a
///   long game the difference is a visible half-degree of drift per level. The
///   original's own values are kept so the colours it produced are the colours
///   this produces.
/// * `int()` in Python truncates *towards zero*, so a value of `-9.6` becomes
///   `-9` and not `-10`. Rust's `as i32` does the same; `.round()` or
///   `.floor()` would each be off by one on half the levels, in opposite
///   directions, which is exactly the kind of difference nobody notices until
///   two screenshots are put side by side.
pub fn level_tint(color: [u8; 3], level: i32) -> [u8; 3] {
    let t = (level as f32) * 0.25;
    let delta = |amp: f32, phase: f32| (amp * (t + phase).sin()) as i32;
    let clamp = |base: u8, d: i32| (base as i32 + d).clamp(0, 255) as u8;
    [
        clamp(color[0], delta(12.0, 0.0)),
        clamp(color[1], delta(8.0, 2.094)),
        clamp(color[2], delta(10.0, 4.189)),
    ]
}

/// The signature glossy, bevelled block from the original: a vertical
/// gradient body, a bright gloss in the upper-left, darker lower-right faces,
/// and a dark outline.
///
/// `level` is threaded here rather than resolved by the caller because the
/// original tinted inside this function: every block on screen - the settled
/// stack, the live piece, the ghost, and the hold and next previews - went
/// through the same one, so there is no block anywhere that is not tinted.
///
/// This is [`Skin::Classic`], the original's only look. The actual drawing
/// lives in [`crate::skins`] alongside the other materials, so a block cannot
/// be drawn one way here and a subtly different way there; this function only
/// resolves the level tint and hands off. `pulse` is the 0..=1 breathing value
/// from [`crate::skins::pulse_for_frame`] - Classic ignores it.
///
/// `keep` is the level-up transition's colour multiplier: `1.0` draws the block
/// normally, anything less drains the body toward ash and re-lights its rim in
/// the undrained colour. The drain and the rim live here rather than in
/// `skins::draw_block` so that all five materials get the level-up treatment
/// without any of them knowing a level-up exists — a skin that had to be taught
/// about the transition is a skin that can forget to implement it.
///
/// The rim is floored by [`fx_keep`], not here. This function takes the body's
/// and the rim's keep values as the pair that was computed, and draws them. It
/// does not recompute the rim from the body, because for [`LevelFx::Rim`] those
/// two are not related at all — that variant's entire effect is the edge, and a
/// renderer that derived the edge from the body would erase it.
pub fn draw_3d_block(
    d: &mut RaylibDrawHandle,
    x: i32,
    y: i32,
    size: i32,
    color: [u8; 3],
    level: i32,
    look: crate::skins::Look,
    keep: (f32, f32),
) {
    let color = level_tint(color, level);
    let (body_keep, rim_keep) = (
        keep.0.clamp(0.0, 1.0),
        keep.1.clamp(0.0, 1.0).max(MIN_RIM_KEEP),
    );
    if body_keep >= 1.0 && rim_keep >= 1.0 {
        crate::skins::draw_block(d, x, y, size, color, look.skin, look.pulse);
        return;
    }
    let body = drain_colour(color, body_keep);
    crate::skins::draw_block(d, x, y, size, body, look.skin, look.pulse);
    let rim = drain_colour(color, rim_keep);
    d.draw_rectangle_lines(
        x,
        y,
        size,
        size,
        Color::new(rim[0], rim[1], rim[2], 255),
    );
}

/// Draw a piece at a pixel origin. `alpha` of 1.0 is solid; lower values dim
/// it toward black, which is how the ghost is drawn. `palette` is the active
/// theme's block palette (index 0 is the empty sentinel).
///
/// `keep` is the level-up drain's `(interior, rim)` multipliers, exactly as for
/// [`draw_3d_block`]. The live piece, its ghost and the previews pass the same
/// pair the stack is using, because they are the same block: a transition that
/// drained the stack while the falling piece stayed fully coloured would look
/// like an animation running behind the game rather than an event happening to
/// it.
///
/// `level` is passed to [`draw_3d_block`] rather than applied here so that the
/// live piece, the ghost and the hold/next previews all tint identically - they
/// are the same block drawn at different sizes, and a tint that applied to only
/// some of them would make a preview's colour a lie about the piece's colour.
pub fn draw_piece(
    d: &mut RaylibDrawHandle,
    piece: Piece,
    rot: Rotation,
    origin: (i32, i32),
    color: u8,
    scale: f32,
    alpha: f32,
    palette: &[[u8; 3]; 8],
    level: i32,
    look: crate::skins::Look,
    keep: (f32, f32),
) {
    let s = ((BLOCK_SIZE as f32) * scale).max(1.0) as i32;
    let (ox, oy) = origin;
    let base = palette[color.clamp(1, 7) as usize];
    for (cx, cy) in pieces::cells(piece, rot) {
        let bx = ox + cx * s;
        let by = oy + cy * s;
        draw_3d_block(d, bx, by, s, base, level, look, keep);
    }
    if alpha < 1.0 {
        for (cx, cy) in pieces::cells(piece, rot) {
            let bx = ox + cx * s;
            let by = oy + cy * s;
            d.draw_rectangle(
                bx,
                by,
                s,
                s,
                Color::new(0, 0, 0, (255.0 * (1.0 - alpha)).clamp(0.0, 255.0) as u8),
            );
        }
    }
}

/// Faint lattice behind the playfield.
pub fn draw_grid_lines(d: &mut RaylibDrawHandle, x: i32, y: i32, w: i32, h: i32, color: Color) {
    let mut gx = x;
    while gx <= x + w {
        d.draw_line_ex(
            Vector2 { x: gx as f32, y: y as f32 },
            Vector2 { x: gx as f32, y: (y + h) as f32 },
            1.0,
            color,
        );
        gx += BLOCK_SIZE;
    }
    let mut gy = y;
    while gy <= y + h {
        d.draw_line_ex(
            Vector2 { x: x as f32, y: gy as f32 },
            Vector2 { x: (x + w) as f32, y: gy as f32 },
            1.0,
            color,
        );
        gy += BLOCK_SIZE;
    }
}

/// Blit the fixed-size game render onto the real framebuffer.
///
/// The game is laid out once in a virtual resolution (`total_w` x
/// `SCREEN_HEIGHT`) and drawn into an off-screen texture, then shown through
/// here. Drawing straight into the framebuffer instead would pin every
/// coordinate to the top-left corner at its original pixel size: maximising or
/// going fullscreen would leave the UI tiny in one corner and the rest of the
/// screen black.
///
/// The image is scaled to fit, centred, and letterboxed with black bars when
/// the aspect ratios differ. Source height is negative because render textures
/// are stored upside down relative to ordinary textures.
pub fn present(
    d: &mut RaylibDrawHandle,
    target: &RenderTexture2D,
    viewport: (i32, i32),
    screen: (i32, i32),
) {
    let (vw, vh) = viewport;
    let (vw, vh) = (vw as f32, vh as f32);
    if vw <= 0.0 || vh <= 0.0 || screen.0 <= 0 || screen.1 <= 0 {
        return;
    }

    let dest = fit(viewport, screen);
    d.draw_texture_pro(
        target,
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: vw,
            height: -vh,
        },
        dest,
        Vector2 { x: 0.0, y: 0.0 },
        0.0,
        Color::WHITE,
    );
}

/// Where the game lands inside a `screen`-sized framebuffer: scaled to fit,
/// centred, with black bars where the aspect ratios disagree.
///
/// Returns the rectangle unchanged when either size is degenerate, so a
/// zero-sized framebuffer during a window drag cannot produce a divide by
/// zero or a NaN rectangle.
pub fn fit(viewport: (i32, i32), screen: (i32, i32)) -> Rectangle {
    let (vw, vh) = (viewport.0 as f32, viewport.1 as f32);
    let (sw, sh) = (screen.0 as f32, screen.1 as f32);
    if vw <= 0.0 || vh <= 0.0 || sw <= 0.0 || sh <= 0.0 {
        return Rectangle {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        };
    }
    let scale = (sw / vw).min(sh / vh);
    let (dw, dh) = (vw * scale, vh * scale);
    Rectangle {
        x: (sw - dw) / 2.0,
        y: (sh - dh) / 2.0,
        width: dw,
        height: dh,
    }
}

/// Playfield: lattice, locked blocks, ghost (with its dark shadow
/// reflection), then the live piece on top.
///
/// The grid has `HIDDEN_ROWS` buffer rows above the visible board for
/// spawning, so the *visible* rows are `HIDDEN_ROWS..TOTAL_ROWS`. They map to
/// pixels row-for-row from the top of the window: row `y` draws at
/// `(y - HIDDEN_ROWS) * BLOCK_SIZE`, which puts the floor on the bottom edge
/// exactly like the Python original's 15x31 well.
///
/// `ghost_alpha` is the fully-resolved ghost opacity (0 = ghost off);
/// `offset` is the screen-shake translation applied to the whole well.
/// `grid` is the theme's lattice colour behind the playfield.
///
/// `fx` is the level-up transition in progress, or `None` when the stack is
/// drawing normally.
///
/// Everything on the playfield drains together — the settled blocks, the live
/// piece, its ghost — via [`piece_keep`] for the pieces and [`fx_keep`] for the
/// grid. They used not to: the pieces were pinned to `(1.0, 1.0)` at the draw
/// call on the reasoning that "the block in flight has to stay maximally
/// legible at 20G". In practice it meant the stack went grey while the piece
/// the player was steering stayed fully coloured, so the transition read as an
/// animation running behind the game rather than an event happening to it.
///
/// The legibility worry is real but it is answered by the rim floor, not by
/// exempting the piece: [`fx_keep`] never lets a rim fall below
/// [`MIN_RIM_KEEP`](crate::config::MIN_RIM_KEEP), so the falling piece keeps a
/// lit outline at every frame of the transition and is never a grey silhouette.
#[allow(clippy::too_many_arguments)]
pub fn draw_board(
    d: &mut RaylibDrawHandle,
    g: &Game,
    palette: &[[u8; 3]; 8],
    ghost_alpha: f32,
    offset: (i32, i32),
    grid: Option<Color>,
    look: crate::skins::Look,
    fx: Option<(LevelFx, f32)>,
) {
    let (ox, oy) = offset;

    // The lattice is drawn behind the blocks. `None` means the player turned
    // it off, or set its opacity to zero (the original's `grid_lines` and
    // `grid_opacity` settings).
    if let Some(colour) = grid {
        draw_grid_lines(d, 0 + ox, 0 + oy, SCREEN_WIDTH, SCREEN_HEIGHT, colour);
    }

    // Where the stack starts and stops, so a block knows its own height in it.
    // Resolved once per frame rather than per cell: it is 33 scans either way,
    // and `fx_keep`'s `Wave` variant is the only consumer.
    let span = fx.and_then(|_| g.grid.occupied_span());
    let rows = span
        .map(|(lo, hi)| (hi.saturating_sub(lo).max(1)) as f32)
        .unwrap_or(1.0);

    for y in HIDDEN_ROWS..TOTAL_ROWS {
        for x in 0..GRID_WIDTH as i32 {
            let c = g.grid.get(x, y as i32);
            if c != 0 {
                let px = x * BLOCK_SIZE + ox;
                let py = (y - HIDDEN_ROWS) as i32 * BLOCK_SIZE + oy;
                // Where this block sits in its stack, `0` at the floor and `1` at
                // the top. Only `Wave` reads it; the other two variants leave it
                // at a value that does not affect them, so there is no reason
                // to branch around the call.
                let height = span
                    .map(|(lo, _)| (y.saturating_sub(lo) as f32 / rows).clamp(0.0, 1.0))
                    .unwrap_or(0.0);
                let (body, rim) = fx
                    .map(|(kind, t)| {
                        fx_keep(kind, t, g.grid.exposure(x, y as i32), height)
                    })
                    .unwrap_or((1.0, 1.0));
                draw_3d_block(
                    d,
                    px,
                    py,
                    BLOCK_SIZE,
                    palette[(c as usize).clamp(1, 7)],
                    g.level,
                    look,
                    (body, rim),
                );
            }
        }
    }

    let piece = g.piece();
    let rot = g.rotation();
    let color = g.color();

    // The falling piece drains on the same clock as the stack. Resolved once
    // here rather than per cell: it does not depend on the cell, and the piece
    // is up to four cells of the same block drawn with the same pair.
    let pkeep = fx.map(|(kind, t)| piece_keep(kind, t)).unwrap_or((1.0, 1.0));

    // Ghost: where the piece would land, if it is not already there.
    let ghost = g.ghost_origin();
    if ghost.1 != g.origin().1 && ghost_alpha > 0.01 {
        let gx = ghost.0 * BLOCK_SIZE + ox;
        let gy = (ghost.1 - HIDDEN_ROWS as i32) * BLOCK_SIZE + oy;
        // The original's dark "shadow reflection" behind the ghost cells.
        for (cx, cy) in pieces::cells(piece, rot) {
            d.draw_rectangle(
                gx + cx * BLOCK_SIZE,
                gy + cy * BLOCK_SIZE,
                BLOCK_SIZE,
                BLOCK_SIZE,
                Color::new(30, 30, 30, 20),
            );
        }
        draw_piece(
            d, piece, rot, (gx, gy), color, 1.0, ghost_alpha, palette, g.level, look, pkeep,
        );
    }

    let px = g.origin().0 * BLOCK_SIZE + ox;
    let py = (g.origin().1 - HIDDEN_ROWS as i32) * BLOCK_SIZE + oy;
    draw_piece(
        d, piece, rot, (px, py), color, 1.0, 1.0, palette, g.level, look, pkeep,
    );
}

fn text(d: &mut RaylibDrawHandle, s: &str, x: i32, y: i32, size: i32, c: Color) {
    d.draw_text(s, x, y, size, c);
}

/// Smallest font size any label is shrunk to before we give up and let it clip.
///
/// Below this a bitmap-font label stops being readable, so a string that cannot
/// fit at this size is a string with too much in it - and the honest failure is
/// a legible one-pixel-oversized label, not an illegible whole list.
const MIN_TEXT_SIZE: i32 = 12;

/// Where a centred label of `width` starts, in virtual-screen pixels.
///
/// Never negative. Centring is `(SCREEN_WIDTH - width) / 2`, which goes negative
/// for any string wider than the screen, and `draw_text` accepts a negative x
/// without complaint - so an over-long label does not look cramped, it silently
/// loses its first few characters off the left edge. That is exactly how
/// "Controller Keybinds: 8 of 8 bound - auto" ended up drawn as
/// "oller Keybinds: 8 of 8 bound - auto".
pub fn centered_x(width: i32) -> i32 {
    ((SCREEN_WIDTH - width) / 2).max(0)
}

/// The largest font size at or below `want` at which the text still fits.
///
/// `measure(size)` is the width in pixels of the text drawn at `size`. Taking
/// it as a parameter rather than measuring inside is what makes this testable at
/// all: real text measurement needs a window, and a layout rule this easy to get
/// wrong should not be one that only runs on a machine with a screen attached.
///
/// One estimate from the width ratio, then step down to correct for rounding.
/// Measuring all the way from `want` to [`MIN_TEXT_SIZE`] one pixel at a time
/// would cost thirty text measurements on every string of every frame, for a
/// font whose widths are very nearly linear in the size.
pub fn fit_size(want: i32, mut measure: impl FnMut(i32) -> i32) -> i32 {
    let at_want = measure(want);
    if at_want <= SCREEN_WIDTH || want <= MIN_TEXT_SIZE {
        return want;
    }
    let mut size = ((SCREEN_WIDTH * want) / at_want).clamp(MIN_TEXT_SIZE, want);
    while size > MIN_TEXT_SIZE && measure(size) > SCREEN_WIDTH {
        size -= 1;
    }
    size
}

/// [`fit_size`] for one string.
fn fit_one(d: &mut RaylibDrawHandle, s: &str, want: i32) -> i32 {
    fit_size(want, |size| d.measure_text(s, size))
}

/// [`fit_size`] for a whole column of rows, measured as one block.
///
/// Sizing the *list* rather than each row is the point. A row that shrank to fit
/// while its neighbours did not reads as a rendering fault, not as a long value;
/// one size for the column keeps the rows the same height as one another and
/// still guarantees none of them is clipped.
fn fit_list(d: &mut RaylibDrawHandle, texts: &[String], want: i32) -> i32 {
    fit_size(want, |size| {
        texts
            .iter()
            .map(|s| d.measure_text(s, size))
            .max()
            .unwrap_or(0)
    })
}

/// The rows as they will actually be drawn, selection markers included.
///
/// The highlighted row grows by four characters, so measuring the bare labels
/// would size the column to fit every row *except* the one the player is
/// currently looking at - which is the one that has to be readable.
fn marked_rows(rows: &[String], selected: usize) -> Vec<String> {
    rows.iter()
        .enumerate()
        .map(|(i, r)| {
            if i == selected {
                format!("> {r} <")
            } else {
                r.clone()
            }
        })
        .collect()
}

/// Draw `s` centred across the virtual screen, shrunk to fit if it is too wide.
fn center(d: &mut RaylibDrawHandle, s: &str, y: i32, want: i32, c: Color) {
    let size = fit_one(d, s, want);
    let w = d.measure_text(s, size);
    d.draw_text(s, centered_x(w), y, size, c);
}

/// The side panel: score, level, counters, hold, next queue, danger warning.
///
/// `keep` is the level-up drain's `(interior, rim)` pair, the same one the board
/// and the falling piece are using. The previews drain with it so the whole
/// screen agrees: a hold slot at full colour while the stack is drained reads as
/// a second, unrelated piece of the UI that never got the memo.
/// Vertical geometry of the clock block in the side panel.
///
/// Named rather than inline so the block cannot overlap what follows it. The
/// clock is a 16 pt label with a 24 pt value drawn [`CLOCK_VALUE_DROP`] px below
/// it, so the drawn block is about 46 px tall; advancing the cursor by the
/// label's own height put the combo counter straight through the digits. The
/// gap below is the slack that stops the two.
const CLOCK_LABEL_SIZE: i32 = 16;
const CLOCK_VALUE_SIZE: i32 = 24;
const CLOCK_VALUE_DROP: i32 = 18;

/// Gap from the row above to the clock's caption.
///
/// Stepped before drawing, so this is the clearance between "Pieces:" and
/// "TIME". Every other row in the panel advances its cursor after drawing; a block
/// that drew at the current position lands on the line above, which is what this
/// one did.
const CLOCK_LABEL_GAP: i32 = 30;

/// The bottom of the drawn clock block, relative to the cursor the block starts
/// at.
///
/// Raylib's default font is roughly `1.5 x` its point size tall including the
/// descender, so the value reaches below the 24 pt it was drawn at. This is the
/// number that has to stay under whatever the panel draws next.
const fn clock_block_bottom() -> i32 {
    CLOCK_VALUE_DROP + CLOCK_VALUE_SIZE * 3 / 2
}

/// Slack between the drawn block and the next row.
///
/// Derived from [`clock_block_bottom`] rather than set by hand, so the reserved
/// space cannot drift away from the space actually drawn - which is precisely how
/// the combo counter ended up on top of the digits. It is pinned by
/// `the_clock_block_reserves_more_than_it_draws`.
const CLOCK_SLACK: i32 = 4;
const CLOCK_BLOCK_H: i32 = clock_block_bottom() + CLOCK_SLACK;

/// `m:ss` from a millisecond count, for the mode clocks.
///
/// Not `{:.1}s`. A sprint finish is the number being raced and a marathon is
/// not, and `142.35s` takes longer to read at a glance than `2:22` does. Rolls
/// over into minutes at 60 rather than at 100, so it is a clock.
fn clock_string(ms: u64) -> String {
    let total = ms / 1000;
    format!("{}:{:02}", total / 60, total % 60)
}

/// [`clock_string`] for tests, which cannot see a private fn from another module.
///
/// A thin re-export rather than making the real one `pub`: the formatter is an
/// implementation detail of the panel, and publishing it would invite the main
/// loop to build its own clock strings with it and drift out of step with the one
/// on screen.
#[cfg(test)]
pub fn clock_string_for_test(ms: u64) -> String {
    clock_string(ms)
}

pub fn draw_panel(
    d: &mut RaylibDrawHandle,
    g: &Game,
    total_w: i32,
    now_s: f64,
    palette: &[[u8; 3]; 8],
    tetris_flash: bool,
    look: crate::skins::Look,
    keep: (f32, f32),
    now_ms: u64,
) {
    let px = SCREEN_WIDTH;
    d.draw_rectangle(px, 0, total_w - px, SCREEN_HEIGHT, Color::new(12, 12, 18, 255));
    d.draw_line_ex(
        Vector2 { x: px as f32, y: 0.0 },
        Vector2 { x: px as f32, y: SCREEN_HEIGHT as f32 },
        1.0,
        Color::new(70, 70, 90, 255),
    );

    let mut y = 30;
    text(d, &format!("Score: {}", g.score), px + 20, y, 20, Color::WHITE);
    y += 34;
    text(d, &format!("Level: {}", g.level), px + 20, y, 20, Color::WHITE);
    y += 30;
    text(
        d,
        &format!("Lines: {}", g.lines_cleared),
        px + 20,
        y,
        18,
        Color::GRAY,
    );
    y += 26;
    text(
        d,
        &format!("Pieces: {}", g.pieces_dropped),
        px + 20,
        y,
        18,
        Color::GRAY,
    );

    // The clock, for every mode that has one. The original draws `m:ss` in the
    // side panel whenever `mode != 'marathon'`, and both timed modes need it for
    // opposite reasons: Sprint is a race and has to show the time it is beating,
    // Ultra is a countdown and has to show what is left of the three minutes.
    // Neither could be played blind.
    //
    // A mode with a *limit* shows the time remaining; one without shows elapsed.
    // Ultra reading `0:00` at the end is right - it is the limit being reached -
    // where Marathon has no limit to reach and so shows no clock at all.
    if g.mode.shows_clock() {
        // Step down *before* drawing, the way the combo and B2B rows below do.
        // Every other row in this panel advances the cursor after drawing and then
        // draws at the new position; drawing at the current one put the "TIME"
        // caption straight on top of the "Pieces:" line above it.
        y += CLOCK_LABEL_GAP;
        let left = g.remaining_ms(now_ms);
        let ms = left.unwrap_or_else(|| g.elapsed_ms(now_ms));
        text(
            d,
            if left.is_some() { "LEFT" } else { "TIME" },
            px + 20,
            y,
            CLOCK_LABEL_SIZE,
            Color::GRAY,
        );
        text(
            d,
            &clock_string(ms),
            px + 20,
            y + CLOCK_VALUE_DROP,
            CLOCK_VALUE_SIZE,
            // The last half-minute of an Ultra reads red. Not a warning that
            // something has gone wrong - a warning that it is about to end.
            if matches!(left, Some(left) if left < 30_000) {
                Color::new(255, 90, 90, 255)
            } else {
                Color::YELLOW
            },
        );
        y += CLOCK_BLOCK_H;
    }

    if g.combo > 0 {
        y += 34;
        text(
            d,
            &format!("Combo x{}", g.combo),
            px + 20,
            y,
            20,
            Color::new(255, 200, 0, 255),
        );
    }
    if g.back_to_back {
        y += 28;
        text(d, "B2B", px + 20, y, 20, Color::new(0, 220, 220, 255));
    }

    y += 60;
    text(d, "HOLD", px + 20, y, 16, Color::GRAY);
    y += 12;
    d.draw_rectangle_lines_ex(
        Rectangle {
            x: (px + 24) as f32,
            y: (y - 4) as f32,
            width: 130.0,
            height: 64.0,
        },
        1.0,
        Color::new(60, 60, 80, 255),
    );
    if let Some(h) = g.hold {
        draw_piece(
            d,
            h,
            0,
            (px + 34, y),
            g.color_for(h),
            0.8,
            1.0,
            palette,
            g.level,
            look,
            keep,
        );
    }

    y += 110;
    text(d, "NEXT", px + 20, y, 16, Color::GRAY);
    y += 12;
    for (i, p) in g.next.iter().enumerate().take(5) {
        let s = if i == 0 { 0.9 } else { 0.7 };
        draw_piece(
            d,
            *p,
            0,
            (px + 34, y),
            g.color_for(*p),
            s,
            1.0,
            palette,
            g.level,
            look,
            keep,
        );
        y += 62;
    }

    // The original's Tetris call-out: within `tetris_flash_time` ms of a
    // four-line clear the side panel shows "TetraFusion!" in a colour that
    // changes on every frame (`random.choice(COLORS)` in draw_subwindow).
    if tetris_flash {
        let mut rng = rand::thread_rng();
        let flash_color = rgb(palette[rng.gen_range(1..=7) as usize]);
        let msg = "TetraFusion!";
        let w = d.measure_text(msg, 28) as i32;
        d.draw_text(msg, px + (total_w - px - w) / 2, SCREEN_HEIGHT - 240, 28, flash_color);
    }

    if g.grid.danger_zone_active(4) {
        let pulse = 0.5 + 0.5 * (now_s as f32 * 6.0).sin();
        center(
            d,
            "DANGER",
            SCREEN_HEIGHT - 60,
            24,
            Color::new(255, (60.0 + 80.0 * pulse) as u8, 60, 255),
        );
    }

    text(d, g.mode.label(), px + 20, SCREEN_HEIGHT - 34, 16, Color::GRAY);
}

/// Big centred banner for pause, win and game-over.
///
/// `hints` are the keys that work on this screen, drawn below the summary. They
/// are not decoration: the game-over screen used to accept `Enter`, `R` and
/// `Escape` and show none of them, so the only way to find out was to lose a run
/// and then guess. Every screen that responds to a key says which key.
pub fn draw_banner(
    d: &mut RaylibDrawHandle,
    title: &str,
    subtitle: Option<&str>,
    hints: &[&str],
    t: f32,
) {
    let a = ((t * 2.0).min(1.0) * 200.0) as u8;
    d.draw_rectangle(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT, Color::new(0, 0, 0, a));

    let pulse = 1.0 + 0.06 * (t * 4.0).sin();
    center(d, title, SCREEN_HEIGHT / 2 - 90, (44.0 * pulse) as i32, Color::WHITE);
    if let Some(sub) = subtitle {
        center(d, sub, SCREEN_HEIGHT / 2 - 20, 20, Color::GRAY);
    }
    // Key hints sit below the summary, spaced far enough apart to scan as three
    // separate lines rather than one paragraph, and dimmed so they read as
    // instructions rather than as part of the result.
    let mut y = SCREEN_HEIGHT / 2 + 30;
    for hint in hints {
        center(d, hint, y, 18, Color::new(170, 170, 185, 255));
        y += 26;
    }
}

/// The level-up banner: the "you got here" moment.
///
/// Three parts, and each one carries weight the others do not. The text punches
/// in from small to large and overshoots before settling, so the arrival has a
/// physical edge instead of just appearing. A bright band sweeps the height of
/// the board, giving the transition a direction to travel in. And the colour is
/// the stage's own palette entry, so the banner is visibly *of* the level the
/// player just reached rather than a generic overlay.
///
/// `t` is `0..=1` across the banner's own life, which is shorter than the
/// drain's: the stack keeps recovering after the text has gone, so the two
/// overlap instead of ending together.
pub fn draw_level_banner(
    d: &mut RaylibDrawHandle,
    level: i32,
    tint: [u8; 3],
    t: f32,
) {
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 1.0 };
    // Cubic ease-out: fast in, slow to settle. A linear ramp reads as a slide,
    // and the punch is the whole point of the first quarter-second.
    const IN: f32 = 0.16;
    let in_t = (t / IN).min(1.0);
    let ease = 1.0 - (1.0 - in_t) * (1.0 - in_t) * (1.0 - in_t);
    // Out over the last third, quadratic so it fades gently at first and then
    // clears quickly rather than lingering as a ghost.
    let out_t = ((t - 0.66) / 0.34).clamp(0.0, 1.0);
    let alpha = (1.0 - out_t * out_t) * (0.3 + 0.7 * ease);
    let size = (30.0 + 44.0 * ease) as i32;
    let c = |m: f32| {
        Color::new(
            (tint[0] as f32 * m).clamp(0.0, 255.0) as u8,
            (tint[1] as f32 * m).clamp(0.0, 255.0) as u8,
            (tint[2] as f32 * m).clamp(0.0, 255.0) as u8,
            (alpha * 255.0).clamp(0.0, 255.0) as u8,
        )
    };

    // The sweep runs the full height over the banner's life, drawn as a thick
    // horizontal band plus a hairline. Two widths because one reads as a bar
    // and the other reads as a line; together they read as a wavefront.
    let sweep_y = (t * SCREEN_HEIGHT as f32) as i32;
    d.draw_rectangle(0, sweep_y - 3, SCREEN_WIDTH, 6, c(1.6));
    d.draw_rectangle(0, sweep_y, SCREEN_WIDTH, 1, c(1.0));

    // A dark plate under the text so it stays readable against a pale stage
    // tint, and so the sweep passing behind it is briefly interrupted.
    let label = format!("LEVEL {level}");
    let w = d.measure_text(&label, size);
    let x = centered_x(w);
    let y = SCREEN_HEIGHT / 2 - 150;
    d.draw_rectangle(
        x - 18,
        y - 10,
        w + 36,
        size + 20,
        Color::new(0, 0, 0, (alpha * 150.0).clamp(0.0, 255.0) as u8),
    );
    d.draw_text(&label, x, y, size, c(1.35));
    // The level's name under it: "Level 7" says what the number is, the
    // gravity line below it says what it *means*.
    center(
        d,
        &format!("{}G", gravity_g(level)),
        y + size + 14,
        20,
        c(0.9),
    );
}

/// The new-record screen: title, what the run scored, the initials field, and
/// the keys that work.
///
/// Shares [`draw_banner`]'s layout so the record screen and the game-over screen
/// are visibly the same family, then puts the entry field between the summary and
/// the hints - the field is the one thing on this screen that the player changes,
/// so it sits where the eye lands after reading the score and before it reaches
/// the instructions.
///
/// The field is drawn from `Initials::display`, which pads to three characters so
/// the line does not shift sideways as letters arrive, and a caret blinks after it
/// whenever there is still room for one.
pub fn draw_initials(
    d: &mut RaylibDrawHandle,
    summary: &str,
    entry: &crate::scores::Initials,
    record: &crate::scores::Record,
    t: f32,
) {
    let shown = entry.display();
    draw_banner(d, "NEW HIGH SCORE", Some(summary), &[], t);

    // The prompt, the current entry, and a blinking caret when there is room.
    let field_y = SCREEN_HEIGHT / 2 + 34;
    let label = "ENTER INITIALS";
    let lw = d.measure_text(label, 18);
    let ew = d.measure_text(&shown, 30);
    let gap = 18;
    let total = lw + gap + ew + 16;
    let x = centered_x(total);
    d.draw_text(label, x, field_y + 8, 18, Color::GRAY);
    d.draw_text(&shown, x + lw + gap, field_y - 4, 30, Color::WHITE);

    // No caret once the field is full: there is nothing left to put in it, and a
    // blinking cursor past three letters suggests a fourth is wanted.
    if !entry.is_full() && (t * 2.0).fract() < 0.5 {
        d.draw_rectangle(
            x + lw + gap + ew + 4,
            field_y - 4,
            3,
            30,
            Color::new(255, 210, 60, 255),
        );
    }

    // What the record is being beaten by, so the number that prompted this screen
    // is still on it.
    center(
        d,
        &format!("BEATS {} ({})", record.score, record.name),
        field_y + 44,
        16,
        Color::new(170, 170, 185, 255),
    );

    let mut y = SCREEN_HEIGHT / 2 + 96;
    for hint in crate::INITIALS_HINTS {
        center(d, hint, y, 18, Color::new(170, 170, 185, 255));
        y += 26;
    }
}

/// The rule for whichever menu entry is highlighted, drawn under the list.
///
/// Separated from [`draw_menu`] rather than folded into it because it is a
/// function of the *mode*, not of the row index: the last two rows are Options and
/// Quit and have no mode to describe, so the caller passes the mode only when a
/// mode is actually selected.
pub fn draw_menu_blurb(d: &mut RaylibDrawHandle, mode: Mode) {
    // Under the last row. `draw_menu` spaces entries 56 px apart starting at
    // y=320, and there are `Mode::ALL.len() + 2` of them, so the last entry's
    // text sits at 320 + 6*56 = 656. Two lines of 15pt text fit below that
    // without reaching the hints at SCREEN_HEIGHT - 140 = 790.
    let y = 320 + (crate::Mode::ALL.len() as i32 + 1) * 56 + 26;
    let blurb = mode.blurb();
    let w = d.measure_text(blurb, 16);
    // A panel behind it, so a long rule does not read as a stray line of game
    // text. Width is clamped to the screen so a rule that outgrows the column is
    // clipped rather than overflowing.
    let pad = 14;
    let panel_w = w.min(SCREEN_WIDTH as i32 - 24) + pad * 2;
    d.draw_rectangle(
        centered_x(panel_w),
        y - 8,
        panel_w,
        32,
        Color::new(30, 30, 44, 255),
    );
    center(d, blurb, y, 16, Color::new(200, 205, 225, 255));

    // The two numbers a mode is really defined by, so a player comparing
    // Marathon against Master is not doing arithmetic in their head.
    let detail = match mode {
        Mode::Sprint => format!("Goal: {} lines", crate::config::SPRINT_TARGET_LINES),
        Mode::Ultra => format!(
            "Limit: {} min",
            crate::config::ULTRA_DURATION_MS / 60_000
        ),
        Mode::Marathon => format!(
            "Level up every {} lines",
            crate::config::LINES_PER_LEVEL
        ),
        Mode::Master => format!(
            "Opens at level {}, {} ms base",
            15,
            crate::game::difficulty_speeds("master")
        ),
        Mode::Training => "H deletes a row".to_string(),
    };
    center(d, &detail, y + 26, 15, Color::GRAY);
}

/// Pause menu with the original's three choices (Resume / Restart /
/// Quit to Menu). Drawn over the dimmed live board exactly like the
/// original's `pause_game` overlay.
pub fn draw_pause_menu(d: &mut RaylibDrawHandle, selected: usize, t: f32) {
    let a = ((t * 3.0).min(1.0) * 200.0) as u8;
    d.draw_rectangle(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT, Color::new(0, 0, 0, a));

    center(d, "PAUSED", SCREEN_HEIGHT / 2 - 100, 40, Color::WHITE);

    let options = ["Resume", "Restart", "Quit to Menu"];
    for (i, opt) in options.iter().enumerate() {
        let y = SCREEN_HEIGHT / 2 - 30 + i as i32 * 48;
        if i == selected {
            center(
                d,
                &format!("> {opt} <"),
                y,
                26,
                Color::new(255, 210, 60, 255),
            );
        } else {
            center(d, opt, y, 26, Color::GRAY);
        }
    }

    center(
        d,
        "UP / DOWN to choose - ENTER to select - P or ESC to resume",
        SCREEN_HEIGHT - 40,
        16,
        Color::GRAY,
    );
}

/// Draw the mode's standing record under the pause overlay.
///
/// The original showed `High: 12400 (ACE)` on its pause subwindow. Without it a
/// record is invisible except for the moment it was set, so a player can never
/// tell what they are chasing; with it, every pause answers "am I close".
pub fn draw_high_score(d: &mut RaylibDrawHandle, mode: crate::game::Mode, table: &crate::scores::Scores) {
    center(
        d,
        &table.line(mode),
        SCREEN_HEIGHT - 68,
        16,
        Color::new(150, 200, 255, 255),
    );
}

/// Main menu with the original's entries (modes plus Options and Quit).
pub fn draw_menu(d: &mut RaylibDrawHandle, selected: usize, labels: &[String], t: f32) {
    center(
        d,
        "TETRAFUSION",
        150,
        52,
        Color::new(0, 230, 230, 255),
    );
    center(d, "2.1 - rust edition", 210, 18, Color::GRAY);

    for (i, label) in labels.iter().enumerate() {
        let y = 320 + i as i32 * 56;
        if i == selected {
            let bob = ((t * 2.0).sin() * 4.0) as i32;
            center(
                d,
                &format!("> {label} <"),
                y + bob,
                30,
                Color::new(255, 210, 60, 255),
            );
        } else {
            center(d, &format!("  {label}  "), y, 30, Color::GRAY);
        }
    }

    center(
        d,
        "UP/DOWN to choose - ENTER to play",
        SCREEN_HEIGHT - 140,
        18,
        Color::GRAY,
    );
    center(
        d,
        "OPTIONS changes the game settings",
        SCREEN_HEIGHT - 110,
        18,
        Color::GRAY,
    );
    center(d, "ESC to quit", SCREEN_HEIGHT - 88, 16, Color::GRAY);
}

/// Where the options rows start and how far apart they sit.
///
/// The options list grew (keybinds, grid lines, grid opacity), so a fixed
/// 60px pitch no longer fits: at 14 rows the last one ran off the bottom of
/// the screen and collided with the footer. Deriving the pitch from the row
/// count keeps every row on screen whatever the list ends up containing.
///
/// Returns `(start_y, pitch)`.
pub fn settings_layout(row_count: usize) -> (i32, i32) {
    const START: i32 = 120;
    const ROW_HEIGHT: i32 = 24;
    const FOOTER_TOP: i32 = SCREEN_HEIGHT - 40;

    if row_count <= 1 {
        return (START, ROW_HEIGHT);
    }
    // Leave a row-height gap above the footer so the last row is not crowded.
    let usable = (FOOTER_TOP - ROW_HEIGHT) - START;
    let pitch = (usable / (row_count as i32 - 1)).max(ROW_HEIGHT);
    (START, pitch)
}

/// The options screen. `rows` already contains each formatted label/value
/// line, and `selected` marks the current one. The footer explains the
/// controls (Enter advances a value, ESC returns).
pub fn draw_settings(d: &mut RaylibDrawHandle, selected: usize, rows: &[String], t: f32) {
    center(d, "OPTIONS", 46, 40, Color::new(255, 210, 60, 255));

    let (start, pitch) = settings_layout(rows.len());
    let display = marked_rows(rows, selected);
    let size = fit_list(d, &display, 24);
    for (i, row) in display.iter().enumerate() {
        let bob = if i == selected {
            ((t * 3.0).sin() * 2.0) as i32
        } else {
            0
        };
        let colour = if i == selected {
            Color::new(255, 210, 60, 255)
        } else {
            Color::GRAY
        };
        center(d, row, start + i as i32 * pitch + bob, size, colour);
    }

    center(
        d,
        "ENTER changes a setting - ESC back to menu",
        SCREEN_HEIGHT - 40,
        18,
        Color::GRAY,
    );
}

/// The Keyboard Keybinds screen: one row per action showing its bound key.
///
/// `capturing` swaps the footer for a prompt, so it is obvious that the next
/// key press is being recorded rather than acted on.
pub fn draw_keybinds(
    d: &mut RaylibDrawHandle,
    selected: usize,
    capturing: bool,
    rows: &[String],
    t: f32,
) {
    center(d, "KEYBOARD KEYBINDS", 46, 40, Color::new(255, 210, 60, 255));

    // The action list is taller than the options screen's 60px pitch allows,
    // so this screen uses a tighter one, as the original's did.
    let start = 120;
    let pitch = 46;
    let display = marked_rows(rows, selected);
    let size = fit_list(d, &display, 22);
    for (i, row) in display.iter().enumerate() {
        let bob = if i == selected && !capturing {
            ((t * 3.0).sin() * 2.0) as i32
        } else {
            0
        };
        let colour = if i == selected {
            Color::new(255, 210, 60, 255)
        } else {
            Color::GRAY
        };
        center(d, row, start + i as i32 * pitch + bob, size, colour);
    }

    if capturing {
        center(
            d,
            "Press any key for this action - ESC to cancel",
            SCREEN_HEIGHT - 40,
            18,
            Color::new(255, 120, 120, 255),
        );
    } else {
        center(
            d,
            "ENTER rebinds - ESC back to options",
            SCREEN_HEIGHT - 40,
            18,
            Color::GRAY,
        );
    }
}

/// The Controller Keybinds screen: one row per action showing its bound
/// controller input, plus the Menu Nav submenu and Back.
///
/// `pad_line` is the connected device's name and slot, or a line saying none was
/// found. It is passed in rather than looked up here because the draw phase
/// cannot re-derive it from the bindings, and a rebinding menu that does not say
/// which device it is listening to is useless to a player with more than one.
pub fn draw_pad_binds(
    d: &mut RaylibDrawHandle,
    selected: usize,
    capturing: bool,
    rows: &[String],
    pad_line: &str,
    t: f32,
) {
    center(d, "CONTROLLER KEYBINDS", 40, 32, Color::new(255, 210, 60, 255));
    // The connected device's name, which can be arbitrarily long.
    center(d, pad_line, 72, 18, Color::GRAY);

    let start = 104;
    let pitch = 44;
    let display = marked_rows(rows, selected);
    // Measured rather than guessed. A hand-picked 20px was a guess that happened
    // to be nearly right for the default bindings and wrong the moment a
    // binding read "Left Stick Y Down" or the player's device name was long.
    let size = fit_list(d, &display, 20);
    for (i, row) in display.iter().enumerate() {
        let bob = if i == selected && !capturing {
            ((t * 3.0).sin() * 2.0) as i32
        } else {
            0
        };
        let colour = if i == selected {
            Color::new(255, 210, 60, 255)
        } else {
            Color::GRAY
        };
        center(d, row, start + i as i32 * pitch + bob, size, colour);
    }

    if capturing {
        center(
            d,
            "Press a button or stick direction - BACK cancels",
            SCREEN_HEIGHT - 40,
            18,
            Color::new(255, 120, 120, 255),
        );
    } else {
        center(
            d,
            "ENTER rebinds - BACK or ESC returns to options",
            SCREEN_HEIGHT - 40,
            18,
            Color::GRAY,
        );
    }
}

/// The Menu Nav Bindings screen: the four menu buttons.
///
/// A separate screen rather than four more rows on the controller screen
/// because these are consulted in every menu and nowhere in play, so they are a
/// different concern: a player binding Select will not be surprised that it does
/// not also change what A does to a piece.
pub fn draw_pad_nav_binds(
    d: &mut RaylibDrawHandle,
    selected: usize,
    capturing: bool,
    rows: &[String],
    t: f32,
) {
    center(
        d,
        "MENU NAV BINDINGS",
        100,
        34,
        Color::new(255, 210, 60, 255),
    );
    center(
        d,
        "used in every menu, nowhere in play",
        140,
        18,
        Color::GRAY,
    );

    let start = 220;
    let pitch = 52;
    let display = marked_rows(rows, selected);
    let size = fit_list(d, &display, 20);
    for (i, row) in display.iter().enumerate() {
        let bob = if i == selected && !capturing {
            ((t * 3.0).sin() * 2.0) as i32
        } else {
            0
        };
        let colour = if i == selected {
            Color::new(255, 210, 60, 255)
        } else {
            Color::GRAY
        };
        center(d, row, start + i as i32 * pitch + bob, size, colour);
    }

    if capturing {
        center(
            d,
            "Press a button or stick direction - BACK cancels",
            SCREEN_HEIGHT - 40,
            18,
            Color::new(255, 120, 120, 255),
        );
    } else {
        center(
            d,
            "ENTER rebinds - BACK returns to controller keybinds",
            SCREEN_HEIGHT - 40,
            18,
            Color::GRAY,
        );
    }
}

/// Flash a label for notable clears, driven by the frame's event list.
pub fn draw_event_flash(d: &mut RaylibDrawHandle, g: &Game, now: f32) {
    let msg: Option<String> = if g.events.iter().any(|e| matches!(e, Event::TSpin)) {
        Some("T-SPIN!".to_string())
    } else if g.events.iter().any(|e| matches!(e, Event::AllClear)) {
        Some("ALL CLEAR!".to_string())
    } else {
        g.events
            .iter()
            .rev()
            .find_map(|e| match e {
                Event::Combo(c) if *c > 0 => Some(format!("COMBO x{c}")),
                _ => None,
            })
    };

    if let Some(m) = msg {
        let a = (now * 2.0).sin() * 0.5 + 0.5;
        center(
            d,
            &m,
            90,
            30,
            Color::new(255, (200.0 - 60.0 * a) as u8, 60, 255),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: (i32, i32) = (819, 930);

    // --- the side panel's clock -----------------------------------------

    /// The clock block has to fit in the space the panel reserves for it.
    ///
    /// It did not. The block was a 16 pt label with a 24 pt value drawn 18 px
    /// below it - 54 px of drawn pixels - and the panel advanced the cursor by the
    /// label's height, so the combo counter was drawn straight through the digits
    /// on any mode that showed both. `CLOCK_BLOCK_H` is now derived from
    /// `clock_block_bottom()`, so the reservation cannot drift from the drawing;
    /// what is left to pin is that there is slack at all, since zero is exactly
    /// what the overlap was.
    #[test]
    fn the_clock_block_reserves_more_than_it_draws() {
        assert!(
            CLOCK_BLOCK_H > clock_block_bottom(),
            "the clock reserves {CLOCK_BLOCK_H} px but draws to {}, so the next row overlaps it",
            clock_block_bottom()
        );
    }

    /// The value's own line box has to clear the label, or the digits sit on the
    /// "TIME"/"LEFT" caption.
    #[test]
    fn the_clock_value_does_not_sit_on_its_own_label() {
        assert!(
            clock_block_bottom() >= CLOCK_VALUE_DROP + CLOCK_VALUE_SIZE,
            "the value ends at {} which is inside its own line box",
            clock_block_bottom()
        );
        assert!(CLOCK_VALUE_DROP >= CLOCK_LABEL_SIZE, "the value overlaps its caption");
    }

    /// The clock caption has to clear the row above it.
    ///
    /// Every other row in `draw_panel` advances the cursor after drawing and draws
    /// at the new position. The clock block drew at the *current* position instead,
    /// so "TIME" was rendered on top of "Pieces:" - visible only in a screenshot,
    /// and only in the four modes that draw a clock. `CLOCK_LABEL_GAP` is the
    /// clearance, and it has to at least clear an 18 pt line.
    #[test]
    fn the_clock_caption_clears_the_row_above_it() {
        const PIECES_ROW_SIZE: i32 = 18;
        assert!(
            CLOCK_LABEL_GAP >= PIECES_ROW_SIZE,
            "the clock caption starts only {CLOCK_LABEL_GAP} px below a {PIECES_ROW_SIZE} px row"
        );
    }

    /// The panel in its tallest case still fits above the mode label.
    ///
    /// The worst case is a timed mode with a live combo *and* back-to-back, which
    /// adds the clock, the combo row and the B2B row to a full five-piece preview.
    /// Asserting it here means adding a row to the panel cannot silently push the
    /// preview off the bottom of the screen.
    #[test]
    fn the_tallest_panel_still_fits_on_screen() {
        // Reproduces draw_panel's vertical walk: Score, Level, Lines, Pieces, the
        // clock block, the optional combo and B2B rows, HOLD, then five previews.
        let mut y = 30;
        y += 34; // Level
        y += 30; // Lines
        y += 26; // Pieces
        y += CLOCK_LABEL_GAP + CLOCK_BLOCK_H; // clock block
        y += 34; // combo
        y += 28; // B2B
        y += 60; // HOLD label
        y += 12 + 64; // hold box
        y += 110; // NEXT label
        y += 12; // first preview
        y += 62 * 5; // five previews

        // The mode label is drawn at the foot of the panel.
        let mode_label_y = SCREEN_HEIGHT - 34;
        assert!(
            y < mode_label_y,
            "the preview stack reaches {y} but the mode label sits at {mode_label_y}"
        );
    }

    /// Without the clock the panel is shorter, so Marathon's panel is the one with
    /// the most room. Both timed modes draw the clock *and* are the ones a player
    /// watches most closely, so the slack is where the clock has to fit - which is
    /// what the tallest-panel assertion above covers.
    #[test]
    fn the_panel_without_a_clock_leaves_more_room_than_the_one_with() {
        fn stack(clock: i32) -> i32 {
            let mut y = 30;
            y += 34 + 30 + 26; // Level, Lines, Pieces
            y += if clock == 0 { 0i32 } else { CLOCK_LABEL_GAP } + clock;
            y += 34 + 28; // combo, B2B
            y += 60 + 12 + 64; // HOLD
            y += 110 + 12 + 62 * 5; // NEXT and five previews
            y
        }
        assert!(stack(0) < stack(CLOCK_BLOCK_H));
        assert!(stack(0) < SCREEN_HEIGHT - 34);
    }

    // --- text fitting ---------------------------------------------------

    /// A stand-in for the real font: `advance` pixels per character, so widths
    /// scale linearly with the size exactly as raylib's default bitmap font's
    /// do. 12.4 is its *measured* advance at 24pt, taken from `MeasureText`.
    fn synthetic(advance_at_24: i32, chars: usize) -> impl FnMut(i32) -> i32 {
        move |size: i32| (chars as i32 * advance_at_24 * size) / 24
    }

    #[test]
    fn text_that_already_fits_is_left_at_its_intended_size() {
        // 29 characters - "Controller Keybinds: 8/8 auto" - measures 360px of
        // the 450px column at 24pt, so the options list renders at 24pt.
        let got = fit_size(24, synthetic(12, 29));
        assert_eq!(got, 24);
    }

    #[test]
    fn text_that_would_be_clipped_is_shrunk_until_it_fits() {
        // The old controller row, "Controller Keybinds: 8 of 8 bound - auto",
        // plus the four selection markers: 45 characters, 560px of a 450px
        // column at 24pt. It has to come back small enough to draw whole.
        let mut measure = synthetic(12, 45);
        let got = fit_size(24, &mut measure);
        assert!(
            got < 24,
            "an overflowing row must shrink, got {got}pt"
        );
        assert!(
            measure(got) <= SCREEN_WIDTH,
            "{got}pt still needs {}px",
            measure(got)
        );
    }

    #[test]
    fn fitting_never_returns_a_size_that_still_overflows() {
        // The loop that walks the estimate down is the part that catches
        // integer-division rounding, so check it across a range of widths
        // rather than trusting one.
        for chars in 1..=200 {
            let mut measure = synthetic(12, chars);
            let got = fit_size(24, &mut measure);
            assert!((MIN_TEXT_SIZE..=24).contains(&got), "size {got} out of range");
            // Either it fits, or it hit the floor and the text is simply too
            // long to be drawn readably - but it must never come back larger
            // than what was asked for.
            assert!(got <= 24, "{chars} chars returned {got}pt");
            if measure(got) > SCREEN_WIDTH {
                assert_eq!(got, MIN_TEXT_SIZE, "{chars} chars: gave up too early");
            }
        }
    }

    #[test]
    fn an_empty_label_does_not_divide_by_zero() {
        // `measure(want)` of 0 is the degenerate input, and it must take the
        // "already fits" branch rather than the ratio estimate.
        assert_eq!(fit_size(24, |_| 0), 24);
    }

    #[test]
    fn a_label_is_never_drawn_off_the_left_edge() {
        // The bug this whole mechanism exists for: `(SCREEN_WIDTH - width) / 2`
        // is negative past the column, and `draw_text` accepts a negative x
        // without complaint, so the label silently loses its first characters.
        assert_eq!(centered_x(0), SCREEN_WIDTH / 2);
        assert_eq!(centered_x(SCREEN_WIDTH), 0);
        assert_eq!(centered_x(SCREEN_WIDTH * 2), 0, "clamped, not negative");
        assert_eq!(centered_x(10_000), 0, "clamped, not negative");
    }

    #[test]
    fn selection_markers_are_part_of_what_gets_measured() {
        // Sizing the column to the bare labels would leave the highlighted row -
        // the one the player is actually reading - four characters too wide.
        let rows = vec!["Grid Opacity: 255".to_string()];
        let bare = marked_rows(&rows, usize::MAX);
        let marked = marked_rows(&rows, 0);
        assert_eq!(bare[0], "Grid Opacity: 255");
        assert_eq!(marked[0], "> Grid Opacity: 255 <");
        assert!(
            synthetic(12, marked[0].chars().count())(24)
                > synthetic(12, bare[0].chars().count())(24),
            "the marked row must measure wider"
        );
    }

    #[test]
    fn the_native_window_size_needs_no_letterboxing() {
        let r = fit(VIEW, VIEW);
        assert_eq!((r.x, r.y, r.width, r.height), (0.0, 0.0, 819.0, 930.0));
    }

    /// A fullscreen 16:9 display is far wider than the game's tall layout, so
    /// height is the binding constraint: the game must fill the window top to
    /// bottom and get equal black bars on the left and right.
    #[test]
    fn a_wide_window_fills_the_height_and_centres_with_side_bars() {
        let r = fit(VIEW, (1920, 1080));
        assert_eq!(r.height, 1080.0, "should fill the full height");
        assert!((r.width - 950.94).abs() < 0.5, "width was {}", r.width);
        assert!(
            (r.x - 484.53).abs() < 0.5 && (r.x + r.width - 1435.47).abs() < 0.5,
            "not centred: x={} right={}",
            r.x,
            r.x + r.width
        );
        assert!((r.x - (1920.0 - r.width) / 2.0).abs() < 0.01);
    }

    /// A tall window is the mirror case: width binds, bars go top and bottom.
    #[test]
    fn a_tall_window_fills_the_width_and_centres_with_bars_top_and_bottom() {
        let r = fit(VIEW, (600, 1400));
        assert_eq!(r.width, 600.0, "should fill the full width");
        assert!((r.height - 681.3).abs() < 0.5, "height was {}", r.height);
        assert!((r.y - (1400.0 - r.height) / 2.0).abs() < 0.01);
    }

    /// A window smaller than the layout must shrink, not crop. Cropping is the
    /// failure mode that hides a bug: everything still draws, just off-screen.
    #[test]
    fn a_smaller_window_shrinks_instead_of_cropping() {
        let r = fit(VIEW, (600, 600));
        assert!(
            r.width <= 600.0 && r.height <= 600.0,
            "overflowed: {}x{}",
            r.width,
            r.height
        );
        assert_eq!(r.height, 600.0, "height binds on a square window");
        assert!((r.x - 35.8).abs() < 0.5, "not centred: x={}", r.x);
    }

    /// The aspect ratio must survive every resize, or the game would stretch.
    #[test]
    fn the_aspect_ratio_is_preserved_at_every_size() {
        let want = VIEW.0 as f32 / VIEW.1 as f32;
        for (w, h) in [
            (320, 240),
            (640, 480),
            (819, 930),
            (1024, 768),
            (1280, 720),
            (1920, 1080),
            (2560, 1440),
            (3840, 2160),
            (3840, 1080),
            (600, 2000),
            (1, 1),
        ] {
            let r = fit(VIEW, (w, h));
            let got = r.width / r.height;
            assert!(
                (got - want).abs() < 0.001,
                "aspect drifted at {w}x{h}: {got} vs {want}"
            );
        }
    }

    /// Never overflow the framebuffer, or the game would be cropped at the
    /// edges by whatever the window manager does next.
    #[test]
    fn the_result_always_stays_inside_the_framebuffer() {
        for (w, h) in [
            (320, 240),
            (640, 480),
            (1024, 768),
            (1280, 720),
            (1920, 1080),
            (3840, 2160),
            (3840, 1080),
        ] {
            let r = fit(VIEW, (w, h));
            assert!(r.x >= 0.0 && r.y >= 0.0, "negative origin at {w}x{h}");
            assert!(
                r.x + r.width <= w as f32 + 0.01,
                "overflows right at {w}x{h}: {}",
                r.x + r.width
            );
            assert!(
                r.y + r.height <= h as f32 + 0.01,
                "overflows bottom at {w}x{h}: {}",
                r.y + r.height
            );
        }
    }

    /// A window drag can report a zero dimension. That must not turn into a
    /// NaN rectangle, which raylib would happily render as garbage.
    #[test]
    fn a_degenerate_framebuffer_is_handled() {
        for bad in [(0, 930), (819, 0), (0, 0), (-5, 100)] {
            let r = fit(VIEW, bad);
            assert_eq!((r.x, r.y, r.width, r.height), (0.0, 0.0, 0.0, 0.0));
            assert!(r.x.is_finite() && r.y.is_finite());
        }
    }

    /// The options list is longer now than the old fixed 60px pitch allowed,
    /// so these guard the layout rather than leaving it to be eyeballed.
    mod settings_layout_tests {
        use super::{settings_layout, SCREEN_HEIGHT};
    // Tied to the real row count so adding an option cannot quietly overflow
    // the screen without one of these tests noticing.
    use crate::SETTINGS_ROWS;

        /// Every row must end above the footer prompt.
        fn assert_fits(row_count: usize) {
            let (start, pitch) = settings_layout(row_count);
            let last = start + (row_count as i32 - 1) * pitch;
            assert!(
                last + 24 <= SCREEN_HEIGHT - 40,
                "{row_count} rows overflow: last row ends at {}, footer at {}",
                last + 24,
                SCREEN_HEIGHT - 40
            );
        }

        #[test]
        fn the_actual_options_list_fits() {
            assert_fits(SETTINGS_ROWS);
        }

        #[test]
        fn a_range_of_row_counts_all_fit() {
            // The list is only ever this long today, but the pitch is derived
            // so a future option cannot silently push rows off the bottom.
            for n in 1..=20 {
                assert_fits(n);
            }
        }

        #[test]
        fn rows_keep_their_order_and_stay_on_screen() {
            let (start, pitch) = settings_layout(SETTINGS_ROWS);
            assert_eq!(start, 120);
            assert!(pitch >= 24, "pitch {pitch} would overlap rows");
            let last = start + (SETTINGS_ROWS as i32 - 1) * pitch;
            assert!(last > start, "rows must not all stack at the top");
            assert!(last + 24 <= SCREEN_HEIGHT - 40);
        }

        #[test]
        fn a_single_row_does_not_divide_by_zero() {
            let (start, pitch) = settings_layout(1);
            assert_eq!(start, 120);
            assert_eq!(pitch, 24);
        }

        #[test]
        fn more_rows_means_tighter_rows_not_smaller_text() {
            let (_, few) = settings_layout(10);
            let (_, many) = settings_layout(18);
            assert!(many <= few, "adding rows must not widen the pitch");
        }
    }

    // --- the per-level colour drift ---------------------------------------

    /// The deltas the original produces, per level, for the first half of a
    /// cycle.
    ///
    /// Computed from `int(12*sin(t))`, `int(8*sin(t+2.094))` and
    /// `int(10*sin(t+4.189))` with `t = level * 0.25`. The point of the table is
    /// that it is the *whole* expected function, not a spot check: three sinusoids
    /// with a truncation in each is exactly the kind of thing that can be right
    /// at level 1 and wrong at level 12, and a spot check would not notice.
    const TINT_DELTAS: [(i32, [i32; 3]); 15] = [
        (0, [0, 6, -8]),
        (1, [2, 5, -9]),
        (2, [5, 4, -9]),
        (3, [8, 2, -9]),
        (4, [10, 0, -8]),
        (5, [11, -1, -7]),
        (6, [11, -3, -5]),
        (7, [11, -5, -3]),
        (8, [10, -6, 0]),
        (9, [9, -7, 1]),
        (10, [7, -7, 3]),
        (11, [4, -7, 6]),
        (12, [1, -7, 7]),
        (13, [-1, -6, 9]),
        (14, [-4, -5, 9]),
    ];

    #[test]
    fn the_level_tint_matches_the_original_level_by_level() {
        // A mid-palette grey, so neither channel starts near a clamp boundary
        // and the delta is the only thing under test.
        let grey = [128, 128, 128];
        for (level, [dr, dg, db]) in TINT_DELTAS {
            assert_eq!(
                level_tint(grey, level),
                [
                    (128 + dr) as u8,
                    (128 + dg) as u8,
                    (128 + db) as u8
                ],
                "level {level}"
            );
        }
    }

    #[test]
    fn the_tint_truncates_towards_zero_rather_than_rounding() {
        // The original's `int()` truncates, so -9.9973 becomes -9 and not -10.
        // Rounding and flooring would each be off by one here, in opposite
        // directions on other levels, so a single-level spot check would have
        // passed for one of them - which is why the table above is checked
        // whole as well.
        //
        // 128 is the base, so the arithmetic in the assertion is visible.
        let grey = [128, 128, 128];
        assert_eq!(level_tint(grey, 2)[2], 128 - 9, "-9.9973 truncates to -9, not -10");
        assert_eq!(level_tint(grey, 6)[0], 128 + 11, "11.9699 truncates to 11, not 12");
        assert_eq!(level_tint(grey, 5)[1], 128 - 1, "-1.6082 truncates to -1, not -2");
    }

    #[test]
    fn the_tint_clamps_instead_of_wrapping() {
        // A dark colour and a bright one, at every level in the table. Without a
        // clamp a -12 delta on a channel of 3 would wrap to 244 and a block
        // would flash near-white once per cycle, which is a bug a player sees
        // immediately and a screenshot of level 1 never shows.
        for (level, [dr, dg, db]) in TINT_DELTAS {
            let want = |base: [u8; 3]| -> [u8; 3] {
                [
                    (base[0] as i32 + dr).clamp(0, 255) as u8,
                    (base[1] as i32 + dg).clamp(0, 255) as u8,
                    (base[2] as i32 + db).clamp(0, 255) as u8,
                ]
            };
            for dark in [[0u8, 0, 0], [3, 5, 7], [12, 0, 12]] {
                let got = level_tint(dark, level);
                assert_eq!(got, want(dark), "level {level} on {dark:?} gave {got:?}");
                // The precise property, stated so a regression cannot pass by
                // producing the right numbers for the wrong reason: a colour
                // near black never comes *out* of black, whatever the delta.
                assert!(
                    got[0] <= dark[0].saturating_add(12)
                        && got[1] <= dark[1].saturating_add(8)
                        && got[2] <= dark[2].saturating_add(10),
                    "level {level} lifted {dark:?} out of range: {got:?}"
                );
            }
            for bright in [[255u8, 255, 255], [250, 254, 253], [243, 255, 250]] {
                let got = level_tint(bright, level);
                assert_eq!(got, want(bright), "level {level} on {bright:?} gave {got:?}");
                assert!(
                    got[0] >= bright[0].saturating_sub(12)
                        && got[1] >= bright[1].saturating_sub(8)
                        && got[2] >= bright[2].saturating_sub(10),
                    "level {level} pushed {bright:?} past white: {got:?}"
                );
            }
        }
        // The two specific wraps this is about, spelled out.
        assert_eq!(level_tint([0, 0, 0], 8)[2], 0, "a -8 delta off zero stays zero");
        assert_eq!(level_tint([255, 255, 255], 6)[0], 255, "a +11 delta off 255 stays 255");
    }

    #[test]
    fn the_tint_returns_the_input_for_a_level_with_no_offset() {
        // Level 0 has a red delta of exactly zero but a non-zero green and blue,
        // so "level 0 is the identity" is *not* true of the original and must not
        // be asserted as such. What must hold is that the tint is a pure shift,
        // so a colour survives unchanged whenever the deltas happen to cancel.
        assert_eq!(level_tint([128, 128, 128], 0)[0], 128);
        assert_ne!(
            level_tint([128, 128, 128], 0),
            [128, 128, 128],
            "the original does drift at level 0"
        );
    }

    #[test]
    fn the_tint_is_gentle_enough_to_read_as_a_shimmer() {
        // The whole point of the amplitudes being 12, 8 and 10: no channel ever
        // moves by more than its own amplitude, in either direction, at any
        // level. If a future edit widens them the palette stops being
        // recognisable between levels, which is the one thing this is not allowed
        // to do - the *palette index* rotating is the change a player should
        // notice, and this is the part that must stay under it.
        //
        // Well past anything reachable in play, because the function is
        // continuous and the point is that it is bounded everywhere.
        for level in -100..=400 {
            let base = [200u8, 100, 40];
            for (c, amplitude) in [12, 8, 10].iter().enumerate() {
                let moved = level_tint(base, level)[c] as i32 - base[c] as i32;
                assert!(
                    moved.abs() <= *amplitude,
                    "level {level} moved channel {c} by {moved}, past {amplitude}"
                );
            }
        }
    }

    #[test]
    fn the_tint_cycles_rather_than_drifting_away() {
        // 2*PI/0.25 is about 25.13 levels, so a full turn is not a whole number
        // of levels and level 25 comes back near - but not exactly on - level 0.
        // The cycle is what stops the palette from walking off over a long game.
        let base = [200u8, 100, 40];
        let zero = level_tint(base, 0);
        let after = level_tint(base, 25);
        for c in 0..3 {
            assert!(
                (after[c] as i32 - zero[c] as i32).abs() <= 2,
                "level 25 is {:?}, not close enough to level 0's {zero:?}",
                after
            );
        }
    }

    #[test]
    fn the_tint_is_pure_so_a_preview_cannot_disagree_with_the_piece() {
        // The hold slot and the next queue draw the same block at a different
        // size, and both pass their own `level` down to `draw_3d_block`. That
        // only works if the tint is a function of the colour and the level and
        // nothing else - no cached state, no dependence on the block size, and
        // no dependence on which call site is asking. Asserted the blunt way:
        // asking the same question twice gives the same answer, and asking it
        // with a different size in between does not perturb it.
        let cyan = [0u8, 230, 230];
        let first = level_tint(cyan, 7);
        for _ in 0..3 {
            assert_eq!(level_tint(cyan, 7), first);
            let _ = level_tint(cyan, 3);
            let _ = level_tint([255, 0, 0], 12);
        }
        // The three palette entries in the same theme, at one level, stay
        // distinguishable - the drift is a few percent, not a wash.
        let palette = [[0u8, 230, 230], [255, 150, 0], [30, 70, 255]];
        let drifted: Vec<[u8; 3]> = palette.iter().map(|c| level_tint(*c, 6)).collect();
        for (i, a) in drifted.iter().enumerate() {
            for (j, b) in drifted.iter().enumerate() {
                if i != j {
                    let apart = (0..3).any(|k| (a[k] as i32 - b[k] as i32).abs() > 60);
                    assert!(apart, "level 6 made entries {i} and {j} look alike: {a:?} {b:?}");
                }
            }
        }
    }
}
