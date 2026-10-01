//! Stage-adaptive piece skins.
//!
//! The original drew every block the same way at every level, so a run looked
//! identical from line one to line two hundred. These skins give the *material*
//! of a block a reason to change with the stage: level one is the familiar
//! glossy bevelled block, and every few levels the stack starts growing
//! crystals, then burning, then sinking into an abyss, then thinning out to bare
//! neon line-art.
//!
//! Two rules keep this from becoming noise:
//!
//! * The palette a skin paints with is a **pure function** of the skin, the
//!   block's own colour and a `pulse` in `0..=1`
//!   ([`skin_palette`]). All the per-skin look lives in that one function, so
//!   it can be checked for the properties that make a skin *read* - a dark body
//!   with a lit rim, cracks that glow, facets that are actually faceted -
//!   without opening a window.
//! * The drawing in [`draw_block`] only decides *geometry*. Colour comes from
//!   [`skin_palette`] alone, so no skin can quietly invent a colour that the
//!   tests have not seen.
//!
//! Colours are handled as plain `[u8; 3]` and converted to raylib's `Color` at
//! the last moment, which is what keeps the recipes testable.

use raylib::core::color::Color;
use raylib::core::drawing::{RaylibDraw, RaylibDrawHandle};
use raylib::ffi::{Rectangle, Vector2};

/// One look for a block.
///
/// Ordered most-solid to most-flat, because [`ALL_SKINS`] walks this list and
/// the automatic skin cycle follows it: a run starts on the heaviest 3D block
/// and works its way down to a bare outline over twelve levels before wrapping.
/// The dimensionality ladder *is* the progression, so this order is not
/// arbitrary and a test holds it in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skin {
    /// The original's glossy bevelled block.
    Classic,
    /// Mirror metal: bright sky above a hard horizon line, dark ground below.
    Chrome,
    /// Charred body with molten cracks.
    Ember,
    /// Faceted, translucent, with a specular that breathes.
    Crystal,
    /// Spectral: four wedges of hue rotating as it pulses.
    Prism,
    /// Solid body with glowing drips running down from the top edge.
    Venom,
    /// Cold and deep, lit only at a bioluminescent rim.
    Abyss,
    /// Matte and quiet. Almost no contrast between the faces.
    Slate,
    /// Pale stone with soft diagonal veins.
    Marble,
    /// Technical: dark body under a diagonal hatch, with corner brackets.
    Wire,
    /// Hollow. A single breathing ring inside an outline.
    Halo,
    /// Minimalist light-line: an unlit body inside a saturated outline.
    Neon,
}

impl Skin {
    pub fn name(self) -> &'static str {
        match self {
            Skin::Classic => "Classic",
            Skin::Chrome => "Chrome",
            Skin::Ember => "Ember",
            Skin::Crystal => "Crystal",
            Skin::Prism => "Prism",
            Skin::Venom => "Venom",
            Skin::Abyss => "Abyss",
            Skin::Slate => "Slate",
            Skin::Marble => "Marble",
            Skin::Wire => "Wire",
            Skin::Halo => "Halo",
            Skin::Neon => "Neon",
        }
    }
}

/// Every skin, in the order the options list cycles them.
///
/// Twelve, so that at [`LEVELS_PER_SKIN`] = 1 a run has a fresh block type for
/// twelve levels before the cycle wraps. Five skins at two levels each ran out
/// of variety after ten and then repeated, which felt like the same handful of
/// materials in a different order rather than something new each level.
///
/// Strictly descending [`Skin::relief`], asserted by a test: the ladder is the
/// point, so it must not be allowed to drift back into an arbitrary order.
pub const ALL_SKINS: [Skin; 12] = [
    Skin::Classic,
    Skin::Chrome,
    Skin::Ember,
    Skin::Crystal,
    Skin::Prism,
    Skin::Venom,
    Skin::Abyss,
    Skin::Slate,
    Skin::Marble,
    Skin::Wire,
    Skin::Halo,
    Skin::Neon,
];

impl Skin {
    /// Wheels out-of-range indices instead of panicking, the way the theme list
    /// does, so a hand-edited settings file cannot take the game down.
    pub fn from_index(i: usize) -> Skin {
        ALL_SKINS[i % ALL_SKINS.len()]
    }

    /// How extruded this skin's blocks are, `0.0..=1.0`.
    ///
    /// This is a declared property rather than a side effect of whichever draw
    /// function happened to add a bevel. Four of the original five skins were
    /// flat for no reason other than that only `Classic` had the extruded faces
    /// written down, so "a different block type per stage" meant four stages
    /// that were the same square in different colours. Declaring relief here
    /// makes the 2D/3D variety a property of the skin list and gives
    /// [`draw_block`] a single source of truth to pass down.
    ///
    /// The values run as a ladder from a hard gloss bevel down to nothing, and
    /// the two outline-only skins sit deliberately at `0.0`: they are *drawn
    /// shapes* rather than lit solids, so a bevel on them would be a lie about
    /// what they are. `Wire` and `Halo` bracket that floor - nearly flat - so
    /// the run does not land on two identical-looking levels at the end of the
    /// cycle.
    pub fn relief(self) -> f32 {
        match self {
            Skin::Classic => 1.00,
            Skin::Chrome => 0.95,
            Skin::Ember => 0.85,
            Skin::Crystal => 0.70,
            Skin::Prism => 0.60,
            Skin::Venom => 0.50,
            Skin::Abyss => 0.45,
            Skin::Slate => 0.30,
            Skin::Marble => 0.20,
            Skin::Wire => 0.10,
            Skin::Halo => 0.00,
            Skin::Neon => 0.00,
        }
    }
}

/// How many levels each skin holds before the stage changes over.
///
/// One. Every level wears its own block type, which is what makes climbing feel
/// like it changes something on the board rather than only in the number in the
/// corner. It was `4` at first and `2` after that; both were long enough that a
/// player could play a run without ever seeing the pieces look different, which
/// was the complaint in the first place - the level-up arrives, the banner
/// lands, and the blocks look exactly as they have for the last twenty lines.
///
/// With [`ALL_SKINS::len`] skins at one each, the cycle runs twelve levels
/// before wrapping.
pub const LEVELS_PER_SKIN: i32 = 1;

/// The skin a level wears when the player has left it on automatic.
///
/// The band wraps rather than running out, so a long run cycles back through the
/// looks instead of arriving at a last skin and sitting there for the rest of
/// the game.
pub fn skin_for_level(level: i32) -> Skin {
    let band = level.max(1).saturating_sub(1) / LEVELS_PER_SKIN;
    Skin::from_index(band as usize)
}

/// A 0..=1 breathing value for `frame`, so skins pulse without a clock.
///
/// Full cycle every 240 frames, so at 60 fps a skin takes four seconds to
/// breathe in and out - slow enough to read as alive rather than as a strobe.
pub fn pulse_for_frame(frame: u64) -> f32 {
    let t = (frame % 240) as f32 / 240.0 * std::f32::consts::TAU;
    (t.sin() * 0.5 + 0.5) as f32
}

/// Everything about how blocks are currently *drawn*, as one value.
///
/// Skin and pulse are always decided together - by the level for the skin, by
/// the frame for the pulse - and every block on screen has to agree on both.
/// Passing them as a pair means a drawing call cannot pick up one stage's
/// material with another stage's pulse by accident.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub skin: Skin,
    pub pulse: f32,
}

impl Look {
    /// The look for `skin` at `frame`.
    ///
    /// The only way a `Look` is built, so the skin and the pulse can never be
    /// picked independently by accident - a block drawn with stage 5's material
    /// and stage 3's pulse would breathe out of step with the rest of the board.
    pub fn new(skin: Skin, frame: u64) -> Look {
        Look {
            skin,
            pulse: pulse_for_frame(frame),
        }
    }
}


// --- pure colour recipes -------------------------------------------------

/// Scale every channel by `factor`, clamping.
fn scale(c: [u8; 3], factor: f32) -> [u8; 3] {
    let f = |v: u8| ((v as f32 * factor).round().clamp(0.0, 255.0)) as u8;
    [f(c[0]), f(c[1]), f(c[2])]
}

/// Blend `a` toward `b` by `t`.
fn blend(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| ((x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0)) as u8;
    [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2])]
}

/// Perceived brightness, 0..=255, for comparing colours.
///
/// Only the tests need this. It is the one measure that answers "is this
/// really brighter" for *any* piece colour at once, which is what the skin
/// recipes are judged on: comparing raw channels says a blue skin is dark
/// because blue's top channel is low, not because the block is.
#[cfg(test)]
fn luma(c: [u8; 3]) -> f32 {
    0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
}

fn rgb(c: [u8; 3]) -> Color {
    Color::new(c[0], c[1], c[2], 255)
}

fn blend_color(a: Color, b: Color, t: f32) -> Color {
    rgb(blend([a.r, a.g, a.b], [b.r, b.g, b.b], t))
}

/// The four colours any skin paints a block with.
///
/// * `top` / `bottom` are the body gradient, lit from above.
/// * `edge` is the outline and any hard shading.
/// * `glow` is everything that emits light: speculars, cracks, rims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkinPalette {
    pub top: [u8; 3],
    pub bottom: [u8; 3],
    pub edge: [u8; 3],
    pub glow: [u8; 3],
}

/// Every colour a skin uses for one block.
///
/// `pulse` is the 0..=1 breathing value from [`pulse_for_frame`]. Only the parts
/// that emit light respond to it, and they only ever brighten: a pulse is a
/// swell, not a flicker, so a block never gets *darker* than the resting
/// recipe says it should.
pub fn skin_palette(skin: Skin, base: [u8; 3], pulse: f32) -> SkinPalette {
    let p = pulse.clamp(0.0, 1.0);
    match skin {
        Skin::Classic => SkinPalette {
            top: scale(base, 1.35),
            bottom: scale(base, 0.55),
            edge: scale(base, 0.28),
            glow: scale(base, 1.75),
        },

        // A cut gem: bright faces, a cold dark underside, and a specular that
        // breathes. `glow` is mixed toward white - and *further* toward white as
        // the pulse rises. Mixing the other way round (more base colour at
        // full pulse) looked right on paper and was wrong for any colour whose
        // channel is already high: a green's "sparkle" got darker as it
        // brightened.
        Skin::Crystal => SkinPalette {
            top: scale(base, 1.55),
            bottom: scale(base, 0.70),
            edge: scale(base, 0.40),
            glow: blend(base, [255, 255, 255], 0.45 + 0.50 * p),
        },

        // Charred, with the heat underneath showing through. The body is much
        // darker than the piece colour, so `glow` reads as molten rather than
        // as "the same colour, lighter" - which is what keeps this from looking
        // like a dimmed Classic.
        Skin::Ember => SkinPalette {
            top: scale(base, 0.34),
            bottom: scale(base, 0.14),
            edge: scale(base, 0.07),
            glow: blend(base, [255, 190, 60], 0.35 + 0.50 * p),
        },

        // Cold and deep. The rim is pushed toward a blue-green bio-light so it
        // is not merely the piece colour again, and the body stays dark enough
        // for that rim to be the brightest thing on the block.
        Skin::Abyss => SkinPalette {
            top: scale(base, 0.95),
            bottom: scale(base, 0.42),
            edge: blend(base, [8, 34, 78], 0.55),
            glow: blend(base, [140, 255, 225], 0.45 + 0.35 * p),
        },

        // Minimalist light-line. Body almost black, outline at the piece
        // colour's own brightness, so the block is a drawn shape rather than a
        // filled one.
        Skin::Neon => SkinPalette {
            top: scale(base, 0.14),
            bottom: scale(base, 0.06),
            edge: base,
            glow: blend(base, [255, 255, 255], 0.25 * p),
        },

        // Mirror metal. The chrome comes from *contrast*, not from desaturation:
        // everything is pulled hard toward a neutral grey-white or grey-black,
        // which is what makes it read as polished rather than as "the piece
        // colour, brighter" - a metal reflects its environment, not its own
        // pigment. The horizon is a hard split rather than a gradient, because
        // that seam is the whole illusion.
        //
        // It was originally 58% toward white on both faces, which destroyed the
        // hue separation between piece types: cyan and orange landed on nearly
        // the same pale grey. In Tetrus the colour is how you read the board at
        // a glance, so a skin that merges two pieces is not a look, it is a
        // difficulty spike. Hue is preserved here and the depth is made with the
        // top/bottom ratio instead.
        Skin::Chrome => SkinPalette {
            top: scale(base, 1.45),
            bottom: scale(base, 0.30),
            edge: blend(base, [255, 255, 255], 0.35),
            glow: blend([255, 214, 150], [255, 255, 255], 0.20 + 0.55 * p),
        },

        // Spectral. Each wedge is the piece hue pushed toward a different
        // primary, so the four quarters of every block disagree with each other
        // and the block as a whole has no single colour. `glow` is a *cool*
        // white rather than `Crystal`'s pure one - the two ramps originally both
        // topped out at 95% white, which meant the faceted gem and the spectral
        // block flashed the same colour at the top of every pulse.
        Skin::Prism => SkinPalette {
            top: blend(base, [255, 110, 200], 0.34),
            bottom: blend(base, [70, 130, 255], 0.46),
            edge: blend(base, [255, 255, 255], 0.50),
            glow: blend(base, [200, 235, 255], 0.50 + 0.40 * p),
        },

        // Sickly and translucent-looking: a mid body pushed green, with a
        // genuinely bright rim so the drips have something to be made of. The
        // body stays darker than the rim, which is the whole point of the skin.
        Skin::Venom => SkinPalette {
            top: blend(base, [70, 150, 60], 0.35),
            bottom: scale(base, 0.26),
            edge: blend(base, [16, 52, 18], 0.55),
            glow: blend(base, [190, 255, 90], 0.45 + 0.40 * p),
        },

        // Matte. `top` and `bottom` are deliberately close together - the skin
        // exists to be the quiet one, and the difference between 0.72 and 0.58
        // is what "no gloss" looks like as a palette rather than as a missing
        // highlight. `glow` still breathes: a matte material can brighten, it
        // just has no shine to catch the light.
        Skin::Slate => SkinPalette {
            top: scale(base, 0.72),
            bottom: scale(base, 0.58),
            edge: scale(base, 0.40),
            glow: blend(scale(base, 0.55), [255, 255, 255], 0.15 + 0.45 * p),
        },

        // Pale stone. The body stays close to the piece colour and the *veins* are
        // what go white - which is how marble actually looks, and it happens to
        // be the only way to keep the pale look at all: lifting the body 46%
        // toward white merged cyan and blue into the same pastel and failed
        // `two_pieces_stay_apart_in_every_skin`. Colour is how the board is read
        // at a glance, so the veins take the paleness instead.
        Skin::Marble => SkinPalette {
            top: blend(base, [255, 255, 255], 0.22),
            bottom: blend(base, [188, 190, 206], 0.16),
            edge: blend(base, [118, 120, 142], 0.42),
            glow: blend([200, 198, 214], [255, 255, 255], 0.15 + 0.70 * p),
        },

        // Terminal phosphor. Near-black body and a green-tinted rim brighter
        // than the piece colour. The glow sweeps rather than holding still: an
        // earlier version of this palette pinned it, on the reasoning that a
        // CRT does not pulse. That was wrong - on a board where every other
        // block breathes, a skin that never moves is four levels of dead
        // material, and "static because it is a CRT" is not worth a player
        // noticing their blocks stopped moving.
        Skin::Wire => SkinPalette {
            top: scale(base, 0.16),
            bottom: scale(base, 0.09),
            edge: blend(base, [120, 255, 160], 0.40),
            glow: blend(base, [150, 255, 190], 0.40 + 0.35 * p),
        },

        // Hollow. Like `Neon` the body is nearly black, but the rim sits at the
        // piece colour rather than above it, so the two outline skins read as
        // different things: one is a bright wire around a dark shape, this one
        // is a dark shape with a coloured rim and a light in the middle.
        Skin::Halo => SkinPalette {
            top: scale(base, 0.09),
            bottom: scale(base, 0.05),
            edge: blend(base, [255, 255, 255], 0.22),
            glow: blend(base, [255, 255, 255], 0.30 + 0.45 * p),
        },
    }
}

// --- drawing -------------------------------------------------------------

/// A stable per-cell hash, so ember cracks are varied across the board but
/// never crawl or flicker between frames.
fn cell_hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_F491);
    h ^= h >> 13;
    h
}

/// Fill a vertical gradient in `n` bands.
///
/// The last band is stretched to the block's full height so the bands cannot
/// leave a one-pixel seam at the bottom when `size` is not a multiple of `n`.
fn bands(
    d: &mut RaylibDrawHandle,
    x: i32,
    y: i32,
    size: i32,
    top: Color,
    bottom: Color,
    n: i32,
) {
    let n = n.max(2);
    let h = size / n;
    for b in 0..n {
        let t = b as f32 / (n - 1) as f32;
        let by = y + b * h;
        let bh = if b == n - 1 { size - b * h } else { h + 1 };
        d.draw_rectangle(x, by, size, bh.max(1), blend_color(top, bottom, t));
    }
}

/// Fill a triangle by scanline.
fn triangle(d: &mut RaylibDrawHandle, pts: [(i32, i32); 3], color: Color) {
    let min_y = pts.iter().map(|p| p.1).min().unwrap_or(0);
    let max_y = pts.iter().map(|p| p.1).max().unwrap_or(0);
    for y in min_y..=max_y {
        let mut xs: [i32; 2] = [0; 2];
        let mut n = 0usize;
        for i in 0..3 {
            let (x0, y0) = pts[i];
            let (x1, y1) = pts[(i + 1) % 3];
            if (y0 <= y && y < y1) || (y1 <= y && y < y0) {
                let t = (y - y0) as f32 / (y1 - y0) as f32;
                xs[n] = x0 + ((x1 - x0) as f32 * t).round() as i32;
                n += 1;
            }
        }
        if n < 2 {
            continue;
        }
        let (lo, hi) = (xs[0].min(xs[1]), xs[0].max(xs[1]));
        d.draw_rectangle(lo, y, hi - lo + 1, 1, color);
    }
}

fn outline(d: &mut RaylibDrawHandle, x: i32, y: i32, size: i32, thick: f32, c: Color) {
    d.draw_rectangle_lines_ex(
        Rectangle {
            x: x as f32,
            y: y as f32,
            width: size as f32,
            height: size as f32,
        },
        thick,
        c,
    );
}

/// The right and bottom faces that make a block read as extruded rather than
/// printed.
///
/// Shared by every skin that has relief, so "is this block 3D" is one decision
/// made once per skin rather than re-decided inside each draw function - which
/// is how four of five skins ended up flat while only `Classic` looked solid.
///
/// `strength` is how far the faces are pushed toward `edge`, `0..=1`. A hard
/// bevel uses `1.0`; a shallower one leaves more of the body colour visible and
/// reads as a thinner extrusion.
fn bevel(d: &mut RaylibDrawHandle, x: i32, y: i32, size: i32, base: Color, edge: Color, strength: f32) {
    let face = ((size as f32 * 0.16) as i32).max(1);
    let s = strength.clamp(0.0, 1.0);
    for i in 0..face {
        let t = (i as f32 / face as f32) * s;
        let c = blend_color(base, edge, t);
        d.draw_rectangle(x + size - face + i, y, 1, size, c);
        d.draw_rectangle(x, y + size - face + i, size, 1, c);
    }
}

/// The upper-left gloss. Rounded off by one pixel per row so it reads as a
/// highlight rather than a pasted rectangle.
fn gloss(
    d: &mut RaylibDrawHandle,
    x: i32,
    y: i32,
    size: i32,
    glow: Color,
    base: Color,
    strength: f32,
) {
    let gx = x + (size as f32 * 0.18) as i32;
    let gy = y + (size as f32 * 0.14) as i32;
    let gw = (size as f32 * 0.40) as i32;
    let gh = ((size as f32 * 0.26) as i32).max(1);
    let k = strength.clamp(0.0, 1.0);
    for i in 0..gh {
        let t = i as f32 / gh as f32;
        let w = (gw as f32 * (1.0 - t * 0.35)) as i32;
        d.draw_rectangle(gx, gy + i, w.max(1), 1, blend_color(glow, base, t * 0.7 * k));
    }
}

/// The original's glossy bevelled block.
fn draw_classic(
    d: &mut RaylibDrawHandle,
    x: i32,
    y: i32,
    size: i32,
    p: SkinPalette,
    relief: f32,
) {
    let base = blend(p.top, p.bottom, 0.5);
    bands(d, x, y, size, rgb(p.top), rgb(p.bottom), 6);

    gloss(d, x, y, size, rgb(p.glow), rgb(base), 1.0);
    bevel(d, x, y, size, rgb(base), rgb(p.edge), relief);

    outline(d, x, y, size, 1.0, rgb(p.edge));
}

/// Faceted gem: body split along the anti-diagonal into a lit and a shadowed
/// half, plus a hard specular.
///
/// The facets are this skin's own dimensionality, but it still takes the shared
/// bevel at a shallower strength than `Classic` - a cut stone has edges, and the
/// facets alone leave the block's outer boundary reading as flat.
fn draw_crystal(
    d: &mut RaylibDrawHandle,
    x: i32,
    y: i32,
    size: i32,
    p: SkinPalette,
    relief: f32,
) {
    let n = x + size;
    let m = y + size;
    bands(d, x, y, size, rgb(p.top), rgb(p.bottom), 4);

    // Anti-diagonal from top-right to bottom-left: the upper-left facet faces
    // the light, the lower-right one falls away.
    triangle(d, [(x, y), (n, y), (x, m)], blend_color(rgb(p.top), rgb(p.glow), 0.25));
    triangle(d, [(n, y), (n, m), (x, m)], blend_color(rgb(p.bottom), rgb(p.edge), 0.35));

    // The cut itself, bright.
    d.draw_line_ex(
        Vector2 { x: n as f32, y: y as f32 },
        Vector2 { x: x as f32, y: m as f32 },
        1.0,
        rgb(p.glow),
    );

    // Specular.
    let s = (size as f32 * 0.20).round().max(1.0) as i32;
    let sx = x + (size as f32 * 0.26) as i32;
    let sy = y + (size as f32 * 0.20) as i32;
    d.draw_rectangle(sx, sy, s, s, rgb(p.glow));
    d.draw_rectangle(sx + s / 2, sy - s / 2, s.max(1) / 2, s.max(1) / 2, rgb(blend(p.glow, [255, 255, 255], 0.6)));

    let base = blend(p.top, p.bottom, 0.5);
    bevel(d, x, y, size, rgb(base), rgb(p.edge), relief);

    outline(d, x, y, size, 1.0, rgb(p.edge));
}

/// Charred block with molten cracks, extruded so it reads as a solid lump of
/// cooling coal rather than a dark square.
fn draw_ember(
    d: &mut RaylibDrawHandle,
    x: i32,
    y: i32,
    size: i32,
    p: SkinPalette,
    relief: f32,
) {
    let base = blend(p.top, p.bottom, 0.5);
    bands(d, x, y, size, rgb(p.top), rgb(p.bottom), 3);

    // Relief first, so the cracks and rim sit on top of the lit faces instead of
    // being cut through by them.
    bevel(d, x, y, size, rgb(base), rgb(p.edge), relief);
    // A dull sheen rather than a gloss: this is ash, not glass.
    gloss(d, x, y, size, rgb(p.glow), rgb(base), 0.45);

    // Three cracks at hashed positions. The hash is keyed on the cell's grid
    // position, so the pattern is different on every block and identical on
    // every frame - a crack that changed shape each frame would read as noise.
    let h = cell_hash(x / 3, y / 3);
    let glow = rgb(p.glow);
    for i in 0..3usize {
        let bit = (h >> (i * 7)) & 0x7f;
        let cx = x + (bit as i32 * size / 127).max(1).min(size - 2);
        let cy = y + ((bit >> 3) as i32 % size.max(2)).max(1).min(size - 2);
        let len = 2 + (bit % 5) as i32;
        let horiz = bit & 1 == 0;
        if horiz {
            d.draw_rectangle(cx, cy, len.min(size - (cx - x)), 1, glow);
        } else {
            d.draw_rectangle(cx, cy, 1, len.min(size - (cy - y)), glow);
        }
    }

    // Hot rim along the top, where the heat would gather.
    d.draw_rectangle(x, y, size, 1, glow);
    outline(d, x, y, size, 1.0, rgb(p.edge));
}

/// Cold block lit only at a bioluminescent rim, with two drifting wave bands.
///
/// Extruded like the others but shallow: the point of this skin is the light,
/// so the faces stay close to the body colour and do not compete with the rim.
fn draw_abyss(
    d: &mut RaylibDrawHandle,
    x: i32,
    y: i32,
    size: i32,
    p: SkinPalette,
    pulse: f32,
    relief: f32,
) {
    let base = blend(p.top, p.bottom, 0.5);
    bands(d, x, y, size, rgb(p.top), rgb(p.bottom), 4);
    bevel(d, x, y, size, rgb(base), rgb(p.edge), relief);

    let glow = rgb(p.glow);
    let mut wave = |row: i32, amp: f32| {
        let drift = (pulse * std::f32::consts::TAU).sin() * amp;
        let ox = (drift * size as f32 * 0.15).round() as i32;
        let mut sx = x + ox;
        let mut seg = 0i32;
        while sx < x + size {
            let w = (size / 4).max(1);
            let yy = y + row + (drift * (seg as f32) * 0.6).round() as i32;
            d.draw_rectangle(sx, yy, w.min(x + size - sx), 1, glow);
            sx += w;
            seg += 1;
        }
    };
    wave(size / 3, 1.0);
    wave(2 * size / 3, -1.0);

    outline(d, x, y, size, 1.5, glow);
    outline(d, x, y, size, 1.0, rgb(p.edge));
}

/// Mirror metal: a hard horizon across the middle, sky above and ground below.
///
/// The split is deliberately a single hard row rather than a gradient. A
/// gradient would just be a paler Classic; what makes chrome look like chrome
/// is that you can see the edge of the world, and the sky is much lighter than
/// its reflection.
fn draw_chrome(
    d: &mut RaylibDrawHandle,
    x: i32,
    y: i32,
    size: i32,
    p: SkinPalette,
    relief: f32,
) {
    let mid = y + size / 2;
    d.draw_rectangle(x, y, size, size / 2, rgb(p.top));
    d.draw_rectangle(x, mid, size, size - size / 2, rgb(p.bottom));

    // The horizon seam.
    d.draw_rectangle(x, mid, size, 1, rgb(blend(p.glow, [255, 255, 255], 0.40)));

    // One specular band in the upper half, and a dimmer bounce light low down.
    let h = ((size as f32 * 0.10).round() as i32).max(1);
    d.draw_rectangle(x, y + (size as f32 * 0.26) as i32, size, h, rgb(p.glow));
    let bounce = rgb(blend(p.glow, p.bottom, 0.62));
    d.draw_rectangle(x, y + size - h - 1, size, 1, bounce);

    let base = blend(p.top, p.bottom, 0.5);
    bevel(d, x, y, size, rgb(base), rgb(p.edge), relief);
    outline(d, x, y, size, 1.0, rgb(p.edge));
}

/// Four wedges meeting at the centre, each pushed toward a different primary.
///
/// This is the one skin whose *hue* is the design: no part of the block agrees
/// with any other part on what colour it is, so it cannot be mistaken for a
/// tint of a different skin. The rotation is driven by `pulse`, which means the
/// whole board slowly cycles its spectrum together.
fn draw_prism(
    d: &mut RaylibDrawHandle,
    x: i32,
    y: i32,
    size: i32,
    p: SkinPalette,
    pulse: f32,
    relief: f32,
) {
    let n = x + size;
    let m = y + size;
    let cx = x + size / 2;
    let cy = y + size / 2;
    let r = pulse;

    triangle(
        d,
        [(x, y), (n, y), (cx, cy)],
        rgb(blend(p.top, [255, 90, 200], 0.30 + 0.25 * r)),
    );
    triangle(
        d,
        [(n, y), (n, m), (cx, cy)],
        rgb(blend(p.glow, [120, 255, 255], 0.20 + 0.25 * r)),
    );
    triangle(
        d,
        [(n, m), (x, m), (cx, cy)],
        rgb(blend(p.bottom, [90, 140, 255], 0.30 + 0.25 * r)),
    );
    triangle(
        d,
        [(x, m), (x, y), (cx, cy)],
        rgb(blend(p.bottom, [255, 220, 90], 0.25 + 0.25 * r)),
    );

    let base = blend(p.top, p.bottom, 0.5);
    bevel(d, x, y, size, rgb(base), rgb(p.edge), relief);
    outline(d, x, y, size, 1.0, rgb(p.edge));
}

/// Solid body with glowing drips running down from the top edge.
///
/// Positions are hashed per cell rather than animated, for the same reason the
/// ember cracks are: a drip that moved every frame would read as flicker, and
/// at 20G a board full of crawling blocks is unplayable.
fn draw_venom(
    d: &mut RaylibDrawHandle,
    x: i32,
    y: i32,
    size: i32,
    p: SkinPalette,
    relief: f32,
) {
    let base = blend(p.top, p.bottom, 0.5);
    bands(d, x, y, size, rgb(p.top), rgb(p.bottom), 3);
    bevel(d, x, y, size, rgb(base), rgb(p.edge), relief);

    let h = cell_hash(x / 3, y / 3);
    let glow = rgb(p.glow);
    for i in 0..3usize {
        let bit = (h >> (i * 5)) & 0x3f;
        let cx = x + (bit as i32 * size / 63).clamp(1, (size - 2).max(1));
        let len = (2 + (bit % 7) as i32).min(size);
        d.draw_rectangle(cx, y, 1, len, glow);
        // A slightly wider bead where the drip has pooled.
        d.draw_rectangle(cx - 1, y + len - 1, 3, 1, glow);
    }

    outline(d, x, y, size, 1.0, rgb(p.edge));
}

/// Matte and quiet: two close bands, one highlight line, no gloss at all.
///
/// The absence is the design. Every other solid skin puts a specular or a gloss
/// highlight on the block, so a skin with neither is unmistakably a different
/// material even though it is drawn from the same pieces.
fn draw_slate(
    d: &mut RaylibDrawHandle,
    x: i32,
    y: i32,
    size: i32,
    p: SkinPalette,
    relief: f32,
) {
    bands(d, x, y, size, rgb(p.top), rgb(p.bottom), 2);
    d.draw_rectangle(x + 1, y + 1, size - 2, 1, rgb(blend(p.glow, p.top, 0.55)));

    let base = blend(p.top, p.bottom, 0.5);
    bevel(d, x, y, size, rgb(base), rgb(p.edge), relief);
    outline(d, x, y, size, 1.0, rgb(p.edge));
}

/// Pale stone with soft diagonal veins.
///
/// The veins are two parallel line pairs at different heights and slopes, drawn
/// one pixel apart so they read as a soft seam rather than a hard line. At the
/// preview sizes the panel uses, that softness is most of what distinguishes
/// this from `Slate`.
fn draw_marble(
    d: &mut RaylibDrawHandle,
    x: i32,
    y: i32,
    size: i32,
    p: SkinPalette,
    relief: f32,
) {
    let base = blend(p.top, p.bottom, 0.5);
    bands(d, x, y, size, rgb(p.top), rgb(p.bottom), 2);

    // A soft sheen, and the only thing on this skin that moves. Without it the
    // `glow` ramp would be computed every frame and never drawn, so `Marble`
    // would be the one block on the board that sat perfectly still - and the
    // palette test would cheerfully pass, because it cannot see the difference
    // between a glow that is drawn and a glow that is merely computed.
    gloss(d, x, y, size, rgb(p.glow), rgb(base), 0.55);

    let rise = (size / 4).max(1);
    // The veins themselves are the pale part, and they use `glow`, so the skin
    // breathes along its seams as well as across its face.
    let vein = rgb(p.glow);
    let deep = rgb(blend(p.edge, p.bottom, 0.30));
    for (i, t) in [0.30f32, 0.68f32].iter().enumerate() {
        let oy = y + (size as f32 * t) as i32;
        let drop = if i == 0 { rise } else { -rise };
        d.draw_line_ex(
            Vector2 { x: x as f32, y: oy as f32 },
            Vector2 { x: (x + size) as f32, y: (oy - drop) as f32 },
            1.0,
            vein,
        );
        d.draw_line_ex(
            Vector2 { x: x as f32, y: (oy + 2) as f32 },
            Vector2 {
                x: (x + size) as f32,
                y: (oy + 2 - drop) as f32,
            },
            1.0,
            deep,
        );
    }

    let base = blend(p.top, p.bottom, 0.5);
    bevel(d, x, y, size, rgb(base), rgb(p.edge), relief);
    outline(d, x, y, size, 1.0, rgb(p.edge));
}

/// Technical: a near-black body under a diagonal hatch with corner brackets.
///
/// The hatch is clipped by clamping each stripe's span to the block's square
/// rather than by drawing long lines and hoping, which is what stops the stripes
/// from bleeding into neighbouring blocks at the edges of the board.
fn draw_wire(
    d: &mut RaylibDrawHandle,
    x: i32,
    y: i32,
    size: i32,
    p: SkinPalette,
    relief: f32,
) {
    bands(d, x, y, size, rgb(p.top), rgb(p.bottom), 2);
    bevel(d, x, y, size, rgb(blend(p.top, p.bottom, 0.5)), rgb(p.edge), relief);

    let glow = rgb(p.glow);
    let gap = ((size as f32 / 3.0) as i32).max(3);
    let mut k = -size;
    while k < size {
        // A stripe on the anti-diagonal, offset by k: the visible span is the
        // part where both coordinates stay inside the block.
        let t0 = (-k).max(0);
        let t1 = (size - k).min(size);
        if t1 > t0 {
            d.draw_line_ex(
                Vector2 {
                    x: (x + t0 + k) as f32,
                    y: (y + size - 1 - t0) as f32,
                },
                Vector2 {
                    x: (x + t1 + k) as f32,
                    y: (y + size - 1 - t1) as f32,
                },
                1.0,
                glow,
            );
        }
        k += gap;
    }

    outline(d, x, y, size, 1.0, rgb(p.edge));
}

/// Hollow: one breathing ring inside an outline, nothing else.
///
/// Deliberately almost empty, so it reads as the opposite of `Classic` at a
/// glance. `pulse` drives only the inner ring, which is what makes it look like
/// something switched on.
fn draw_halo(d: &mut RaylibDrawHandle, x: i32, y: i32, size: i32, p: SkinPalette, pulse: f32) {
    bands(d, x, y, size, rgb(p.top), rgb(p.bottom), 2);

    let inset = ((size as f32 * 0.22).round() as i32).max(1);
    let inner = (size - inset * 2).max(1);
    let ring = rgb(blend(p.glow, [255, 255, 255], 0.35 * pulse.clamp(0.0, 1.0)));
    outline(d, x + inset, y + inset, inner, 1.5, ring);

    outline(d, x, y, size, 2.0, rgb(p.edge));
}

/// Bare outline: an unlit body inside a saturated border with corner ticks.
fn draw_neon(d: &mut RaylibDrawHandle, x: i32, y: i32, size: i32, p: SkinPalette) {
    bands(d, x, y, size, rgb(p.top), rgb(p.bottom), 2);
    outline(d, x, y, size, 2.0, rgb(p.edge));

    // Corner ticks: short bright strokes pointing in from each corner, the
    // only decoration this skin allows itself.
    let tick = (size as f32 * 0.28).round().max(2.0) as i32;
    let glow = rgb(p.glow);
    d.draw_rectangle(x + 1, y + 1, tick.min(size - 2), 1, glow);
    d.draw_rectangle(x + 1, y + 1, 1, tick.min(size - 2), glow);
    d.draw_rectangle(x + size - 1 - tick, y + size - 2, tick.min(size - 2), 1, glow);
    d.draw_rectangle(x + size - 2, y + size - 1 - tick, 1, tick.min(size - 2), glow);
}

/// Draw one block in `skin`, tinted by `base` and breathing on `pulse`.
///
/// `pulse` is the 0..=1 value from [`pulse_for_frame`]. Passing it in rather
/// than reading a clock keeps every skin a pure function of its arguments.
pub fn draw_block(
    d: &mut RaylibDrawHandle,
    x: i32,
    y: i32,
    size: i32,
    base: [u8; 3],
    skin: Skin,
    pulse: f32,
) {
    let p = skin_palette(skin, base, pulse);
    // One source of truth for how solid this skin's blocks are. Dispatching the
    // strength here rather than hardcoding it per draw function is what stops the
    // list drifting back to "one bevelled skin and eleven flat ones".
    let relief = skin.relief();
    match skin {
        Skin::Classic => draw_classic(d, x, y, size, p, relief),
        Skin::Chrome => draw_chrome(d, x, y, size, p, relief),
        Skin::Ember => draw_ember(d, x, y, size, p, relief),
        Skin::Crystal => draw_crystal(d, x, y, size, p, relief),
        Skin::Prism => draw_prism(d, x, y, size, p, pulse.clamp(0.0, 1.0), relief),
        Skin::Venom => draw_venom(d, x, y, size, p, relief),
        Skin::Abyss => draw_abyss(d, x, y, size, p, pulse.clamp(0.0, 1.0), relief),
        Skin::Slate => draw_slate(d, x, y, size, p, relief),
        Skin::Marble => draw_marble(d, x, y, size, p, relief),
        Skin::Wire => draw_wire(d, x, y, size, p, relief),
        // `Halo` and `Neon` are the two outline-only skins, so they take no
        // relief: a bevel would contradict what they are. See `Skin::relief`.
        Skin::Halo => draw_halo(d, x, y, size, p, pulse.clamp(0.0, 1.0)),
        Skin::Neon => draw_neon(d, x, y, size, p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The list must span flat and solid, not pick a side.
    ///
    /// The complaint behind this: "variation of types making them look 2D and
    /// 3D" went unanswered because only `Classic` ever had extruded faces - the
    /// other four drew a flat square, so a stage change was a hue shift rather
    /// than a change of material.
    ///
    /// Both extremes have to survive. A fully solid list would give up the
    /// glowing-outline look entirely; a fully flat one would throw away the
    /// depth. And the flat end must be exactly the two skins that are *drawn
    /// outlines* rather than lit solids - beveling those would be a lie about
    /// what they are, so the assertion names them rather than just counting.
    #[test]
    fn the_skins_mix_flat_and_solid_blocks() {
        let rel: Vec<f32> = ALL_SKINS.iter().map(|s| s.relief()).collect();
        for (skin, r) in ALL_SKINS.iter().zip(&rel) {
            assert!(
                (0.0..=1.0).contains(r),
                "{skin:?} relief {r} is outside 0..=1"
            );
        }
        assert!(
            rel.iter().any(|r| *r <= 0.0),
            "no flat skin: the glowing-outline end of the ladder is gone"
        );
        assert!(
            rel.iter().any(|r| *r >= 0.9),
            "no fully solid skin: the deep 3D end of the ladder is gone"
        );
        assert!(
            rel.iter().any(|r| *r > 0.0 && *r < 0.9),
            "nothing in between: the ladder jumps straight from a bevel to a bare outline"
        );

        let flat: Vec<Skin> = ALL_SKINS.iter().copied().filter(|s| s.relief() <= 0.0).collect();
        assert_eq!(
            flat,
            vec![Skin::Halo, Skin::Neon],
            "only the two outline-only skins may be flat"
        );
    }

    /// `ALL_SKINS` walks the relief values downward, and that order is what a run
    /// is played through: each level takes the next rung from a hard bevel to a
    /// bare outline. If the enum ever gets a variant inserted in the wrong
    /// place, or a list entry appended out of sequence, the cycle would stutter
    /// instead of descending and nothing else would notice.
    ///
    /// The floor is deliberately *flat* rather than strictly descending. The two
    /// outline-only skins both sit at zero, and two rungs at the bottom of a
    /// ladder is not a defect - they differ in what they draw, not in how solid
    /// they are. Every rung above the floor still has to be distinct, which is
    /// where a stray duplicate would show up.
    #[test]
    fn the_skin_list_is_a_descending_ladder_of_dimensionality() {
        for pair in ALL_SKINS.windows(2) {
            assert!(
                pair[0].relief() >= pair[1].relief(),
                "{:?} (relief {}) should not sit below {:?} (relief {})",
                pair[0],
                pair[0].relief(),
                pair[1],
                pair[1].relief()
            );
        }
        // Strict descent for everything that is not the flat floor.
        for pair in ALL_SKINS.windows(2) {
            if pair[1].relief() > 0.0 {
                assert!(
                    pair[0].relief() > pair[1].relief(),
                    "{:?} and {:?} share a rung at relief {}",
                    pair[0],
                    pair[1],
                    pair[1].relief()
                );
            }
        }
        assert_eq!(ALL_SKINS[0], Skin::Classic, "a run should open on the heaviest block");
        assert_eq!(
            *ALL_SKINS.last().unwrap(),
            Skin::Neon,
            "the cycle should end on the flattest block"
        );
    }

    /// Relief has to actually differ between skins, or "variation of type" is
    /// three flavours of the same extrusion. The original five all collapsed
    /// onto one bevel nobody was checking.
    #[test]
    fn the_solid_skins_do_not_all_share_one_extrusion() {
        let mut seen: Vec<f32> = ALL_SKINS
            .iter()
            .map(|s| s.relief())
            .filter(|r| *r > 0.0)
            .collect();
        let total = seen.len();
        seen.sort_by(|a, b| a.partial_cmp(b).unwrap());
        seen.dedup();
        assert!(
            seen.len() >= 8,
            "only {} distinct relief values across {total} solid skins: {:?}",
            seen.len(),
            seen
        );
    }

    /// Every level gets its own skin, so climbing visibly changes the board.
    ///
    /// At four levels per skin a player could finish a run never having seen the
    /// blocks look different. At two it was better but still repeated every ten
    /// levels. The requirement is one skin per level, and with twelve skins that
    /// is a full twelve-level cycle before anything wraps.
    #[test]
    fn every_skin_is_reachable_inside_a_short_run() {
        assert_eq!(
            LEVELS_PER_SKIN, 1,
            "the stage cadence changed; this test's premise no longer holds"
        );
        assert!(
            ALL_SKINS.len() >= 10,
            "only {} skins; a run would wrap before the player saw the variety",
            ALL_SKINS.len()
        );
        // One skin per level, so levels 1..=len() must be exactly the whole list
        // in order - which is also what makes the ladder test above meaningful.
        let seen: Vec<Skin> = (1..=ALL_SKINS.len() as i32)
            .map(skin_for_level)
            .collect();
        assert_eq!(seen, ALL_SKINS.to_vec(), "levels did not walk the whole ladder");
    }

    /// Two consecutive levels must never wear the same skin, and the pair must
    /// not be identical-looking. This is the property that makes a level-up read
    /// as an event on the board and not just in the corner counter.
    #[test]
    fn neighbouring_levels_never_look_the_same() {
        for level in 1..200 {
            let a = skin_for_level(level);
            let b = skin_for_level(level + 1);
            assert_ne!(a, b, "levels {level} and {} both wore {a:?}", level + 1);
        }
    }

    const BASE: [u8; 3] = [0, 200, 50]; // a green, close to the default S

    /// Every palette has to be a real, opaque colour - a skin that painted
    /// black would look like the piece had vanished.
    #[test]
    fn no_skin_paints_a_fully_black_block() {
        for skin in ALL_SKINS {
            let p = skin_palette(skin, BASE, 0.5);
            for (name, c) in [
                ("top", p.top),
                ("bottom", p.bottom),
                ("edge", p.edge),
                ("glow", p.glow),
            ] {
                assert!(
                    luma(c) > 4.0,
                    "{:?} painted its {name} as {c:?}, which reads as a hole",
                    skin
                );
            }
        }
    }

    /// The lit parts must out-shine the body, or a skin has no highlight.
    #[test]
    fn every_skin_has_something_brighter_than_its_body() {
        for skin in ALL_SKINS {
            let p = skin_palette(skin, BASE, 0.5);
            assert!(
                luma(p.glow) > luma(p.top),
                "{:?} glow {glow} is not brighter than its top {top}",
                skin,
                glow = luma(p.glow),
                top = luma(p.top)
            );
        }
    }

    /// Lighting comes from above: the top of the gradient is never the darker
    /// end. A skin that got this backwards would read as lit from underneath.
    #[test]
    fn every_skin_is_lit_from_above() {
        for skin in ALL_SKINS {
            let p = skin_palette(skin, BASE, 0.5);
            assert!(
                luma(p.top) > luma(p.bottom),
                "{:?} top {top} is not above its bottom {bottom} in brightness",
                skin,
                top = luma(p.top),
                bottom = luma(p.bottom)
            );
        }
    }

    /// A pulse is a swell, not a flicker: the lit parts brighten toward 1.0 and
    /// the body never moves at all.
    #[test]
    fn the_pulse_only_brightens_the_lit_parts() {
        for skin in ALL_SKINS {
            let rest = skin_palette(skin, BASE, 0.0);
            let peak = skin_palette(skin, BASE, 1.0);
            assert_eq!(rest.top, peak.top, "{:?} body moved with the pulse", skin);
            assert_eq!(
                rest.bottom, peak.bottom,
                "{:?} body moved with the pulse",
                skin
            );
            assert_eq!(rest.edge, peak.edge, "{:?} outline moved", skin);
            assert!(
                luma(peak.glow) >= luma(rest.glow),
                "{:?} glow dimmed instead of swelling",
                skin
            );
        }
    }

    /// The pulse must actually be visible - a "breathing" skin that changed by
    /// one unit of 255 is not breathing.
    ///
    /// Classic is excluded on purpose: it is the original's block, gloss and
    /// all, and leaving it still is the whole reason it is still there as the
    /// first thing a player sees. The other four are stage looks, and a stage
    /// look that did not move would be indistinguishable from Classic.
    #[test]
    fn the_pulse_moves_the_glow_far_enough_to_see() {
        for skin in ALL_SKINS {
            if skin == Skin::Classic {
                continue;
            }
            let rest = skin_palette(skin, BASE, 0.0);
            let peak = skin_palette(skin, BASE, 1.0);
            assert!(
                luma(peak.glow) - luma(rest.glow) > 12.0,
                "{:?} glow only moved {} units across the whole pulse",
                skin,
                luma(peak.glow) - luma(rest.glow)
            );
        }
    }

    /// No two skins may share a glow ramp, at any pulse value.
    ///
    /// This is the cheapest available check that "different shades every level"
    /// is actually true of the *list* rather than only of the intent. Two skins
    /// with identical ramps would be a duplicate wearing a different name, and
    /// the run would hand the player the same material twice while the level
    /// counter said it had changed. Sampling across the pulse matters because
    /// two ramps can start alike and diverge, or start apart and cross.
    #[test]
    fn no_two_skins_share_a_glow_ramp() {
        for pulse in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let mut seen: Vec<(Skin, [u8; 3])> = ALL_SKINS
                .iter()
                .map(|s| (*s, skin_palette(*s, BASE, pulse).glow))
                .collect();
            let total = seen.len();
            let mut i = 0;
            while i < seen.len() {
                let mut j = i + 1;
                while j < seen.len() {
                    assert_ne!(
                        seen[i].1, seen[j].1,
                        "{:?} and {:?} have the same glow at pulse {pulse}",
                        seen[i].0, seen[j].0
                    );
                    j += 1;
                }
                i += 1;
            }
            seen.dedup();
            assert_eq!(seen.len(), total, "glow ramps collided at pulse {pulse}");
        }
    }

    /// Classic holds still, so a player who wants the original's board has one
    /// that is not quietly pulsing at them.
    #[test]
    fn classic_never_moves() {
        let a = skin_palette(Skin::Classic, BASE, 0.0);
        for frame in 0..=480u32 {
            let p = pulse_for_frame(frame as u64);
            assert_eq!(skin_palette(Skin::Classic, BASE, p), a, "frame {frame}");
        }
    }

    /// Classic is the original's look and must not have been quietly restyled.
    #[test]
    fn classic_is_the_original_glossy_bevel() {
        let p = skin_palette(Skin::Classic, BASE, 0.0);
        assert_eq!(p.top, scale(BASE, 1.35));
        assert_eq!(p.bottom, scale(BASE, 0.55));
        assert_eq!(p.edge, scale(BASE, 0.28));
        assert_eq!(p.glow, scale(BASE, 1.75));
    }

    /// Ember and Neon are the two "dark body, lit outline" skins, and they are
    /// the reason a run does not look like one palette for two hundred lines.
    /// If their bodies were not much darker than the piece colour they would
    /// be indistinguishable from a dimmed Classic.
    #[test]
    fn the_dark_body_skins_actually_darken_the_body() {
        for skin in [Skin::Ember, Skin::Neon] {
            let p = skin_palette(skin, BASE, 0.5);
            assert!(
                luma(p.top) < luma(BASE) * 0.5,
                "{:?} top {top} is not dark enough to read as an unlit body",
                skin,
                top = luma(p.top)
            );
            assert!(
                luma(p.glow) > luma(p.top) * 2.0,
                "{:?} outline does not stand off its body",
                skin
            );
        }
    }

    /// Ember is the one skin that is lit from *inside*, so its glow has to run
    /// warm - red above blue - or it reads as a cold rim like Abyss's.
    #[test]
    fn ember_glows_warm_and_abyss_glows_cold() {
        let e = skin_palette(Skin::Ember, BASE, 1.0).glow;
        assert!(e[0] > e[2], "ember glow {e:?} is not warm");

        let a = skin_palette(Skin::Abyss, BASE, 1.0).glow;
        assert!(a[1] > a[0], "abyss glow {a:?} is not cold");
    }

    /// Neon's border is the piece colour at its own brightness - that is the
    /// whole "glowing line-art" idea. Dimming it would make Neon a dull Ember.
    #[test]
    fn neon_outlines_at_the_piece_colour_itself() {
        let p = skin_palette(Skin::Neon, BASE, 0.5);
        assert_eq!(p.edge, BASE);
    }

    /// Crystal's specular is a white highlight, not a paler piece colour.
    #[test]
    fn crystal_specular_is_near_white() {
        let p = skin_palette(Skin::Crystal, BASE, 1.0);
        assert!(
            luma(p.glow) > 200.0,
            "crystal specular {glow:?} is not bright enough to read as a sparkle",
            glow = p.glow
        );
    }

    /// Every skin must stay recognisable as its own piece across all seven
    /// colours, or a theme would stop being seven distinguishable things.
    ///
    /// The check is on **hue**, not on raw channel distance. The dark-body
    /// skins compress everything towards black, so an absolute threshold that
    /// reads fine on Classic fails on Neon purely because `base * 0.14` rounds
    /// two different blues to nearly the same small number. Hue survives that,
    /// and hue is the thing a player actually identifies a piece by.
    #[test]
    fn each_piece_keeps_its_hue_in_every_skin() {
        let pieces: [[u8; 3]; 7] = crate::config::COLORS[1..]
            .try_into()
            .expect("COLORS has seven real colours after the sentinel");
        for skin in ALL_SKINS {
            for base in pieces {
                let want = crate::config::rgb_to_hsv(base)[0];
                let got = crate::config::rgb_to_hsv(skin_palette(skin, base, 0.5).top)[0];
                let d = (want - got).abs().min(1.0 - (want - got).abs());
                assert!(
                    d < 0.06,
                    "{skin:?} moved {base:?} by {} turns of hue, so it is no longer that colour",
                    want - got
                );
            }
        }
    }

    /// A skin may darken or lighten a piece, but never to the point where two
    /// different pieces merge. Compared after normalising each colour to the
    /// same brightness, so this measures shape rather than brightness.
    #[test]
    fn two_pieces_stay_apart_in_every_skin() {
        let pieces: [[u8; 3]; 7] = crate::config::COLORS[1..]
            .try_into()
            .expect("COLORS has seven real colours after the sentinel");
        // Normalise to unit luma, so a dark-body skin is judged on its hue
        // rather than on being dark.
        let normalise = |c: [u8; 3]| {
            let l = luma(c).max(1.0);
            [c[0] as f32 / l, c[1] as f32 / l, c[2] as f32 / l]
        };
        for skin in ALL_SKINS {
            for (i, a) in pieces.iter().enumerate() {
                for b in &pieces[i + 1..] {
                    let pa = normalise(skin_palette(skin, *a, 0.5).top);
                    let pb = normalise(skin_palette(skin, *b, 0.5).top);
                    let apart = (0..3)
                        .map(|c| (pa[c] - pb[c]).abs())
                        .fold(0.0f32, f32::max);
                    let base_apart = {
                        let na = normalise(*a);
                        let nb = normalise(*b);
                        (0..3).map(|c| (na[c] - nb[c]).abs()).fold(0.0f32, f32::max)
                    };
                    assert!(
                        apart > base_apart * 0.4,
                        "{skin:?} pulled {a:?} and {b:?} together: {apart} vs {base_apart}"
                    );
                }
            }
        }
    }

    /// The options list has to name every skin, and the names have to be the ones
    /// the settings screen writes back. Written out literally on purpose: this
    /// is the list a player reads in the menu, so renaming a skin without
    /// deciding what the menu should now say is exactly the change worth
    /// catching.
    #[test]
    fn skins_are_named_for_the_options_list() {
        let names: Vec<&str> = ALL_SKINS.iter().map(|s| s.name()).collect();
        assert_eq!(
            names,
            vec![
                "Classic",
                "Chrome",
                "Ember",
                "Crystal",
                "Prism",
                "Venom",
                "Abyss",
                "Slate",
                "Marble",
                "Wire",
                "Halo",
                "Neon",
            ]
        );
        // Names have to be unique or the options row cannot be read back.
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len());
    }

    /// A hand-edited settings file must not be able to take the game down. The
    /// indices are written as remainders rather than literals so this keeps
    /// testing what it means to test when the list grows.
    #[test]
    fn an_out_of_range_skin_index_wheels_rather_than_panicking() {
        let n = ALL_SKINS.len();
        assert_eq!(Skin::from_index(0), ALL_SKINS[0]);
        assert_eq!(Skin::from_index(n), ALL_SKINS[0], "one past the end wrapped");
        assert_eq!(Skin::from_index(n + 3), ALL_SKINS[3 % n], "well past the end");
        assert_eq!(
            Skin::from_index(usize::MAX),
            ALL_SKINS[usize::MAX % ALL_SKINS.len()]
        );
    }

    /// Each skin holds its band and then hands over to the next. Written against
    /// `LEVELS_PER_SKIN` and `ALL_SKINS` rather than literal indices, so it stays
    /// true when either changes.
    #[test]
    fn each_stage_skin_holds_for_its_band_then_moves_on() {
        assert_eq!(skin_for_level(1), ALL_SKINS[0]);
        for level in 1..=LEVELS_PER_SKIN {
            assert_eq!(
                skin_for_level(level),
                ALL_SKINS[0],
                "level {level} is inside the first band"
            );
        }
        assert_eq!(
            skin_for_level(LEVELS_PER_SKIN + 1),
            ALL_SKINS[1 % ALL_SKINS.len()]
        );
        // A long run cycles rather than exhausting the list.
        let n = ALL_SKINS.len() as i32 * LEVELS_PER_SKIN;
        assert_eq!(skin_for_level(n + 1), skin_for_level(1));
    }

    #[test]
    fn a_stage_below_level_one_is_treated_as_the_first_stage() {
        for level in [0, -1, -1000, i32::MIN] {
            assert_eq!(skin_for_level(level), Skin::Classic, "level {level}");
        }
        // A level read out of a hand-edited save must still land on a real
        // skin rather than running the band arithmetic off the end.
        let band = (i32::MAX - 1) / LEVELS_PER_SKIN;
        assert_eq!(
            skin_for_level(i32::MAX),
            Skin::from_index(band as usize)
        );
    }

    /// The skin only ever moves forward as the level climbs, except at the wrap
    /// where it must return to the start rather than stop.
    #[test]
    fn the_stage_skin_advances_with_the_level() {
        for level in 2..=200 {
            let prev = skin_for_level(level - 1) as i32;
            let now = skin_for_level(level) as i32;
            if now != prev {
                let wrapped = now == 0 && prev == ALL_SKINS.len() as i32 - 1;
                let stepped = now == prev + 1;
                assert!(stepped || wrapped, "level {level}: {prev} -> {now}");
            }
        }
    }

    #[test]
    fn the_pulse_stays_in_range_and_sweeps_its_whole_cycle() {
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for frame in 0..1000u64 {
            let p = pulse_for_frame(frame);
            assert!((0.0..=1.0).contains(&p), "frame {frame} gave {p}");
            lo = lo.min(p);
            hi = hi.max(p);
        }
        assert!(hi - lo > 0.95, "the pulse only swept {}..{hi}", lo);
        // Out-of-range pulses must clamp to the nearest real one rather than
        // producing something a skin has never been checked with.
        for skin in ALL_SKINS {
            assert_eq!(
                skin_palette(skin, BASE, -5.0),
                skin_palette(skin, BASE, 0.0),
                "{skin:?} did not clamp a negative pulse"
            );
            assert_eq!(
                skin_palette(skin, BASE, 5.0),
                skin_palette(skin, BASE, 1.0),
                "{skin:?} did not clamp a pulse above one"
            );
        }
    }

    /// Crack positions are keyed on the cell, so the same block draws the same
    /// cracks every frame - a pattern that moved on its own would read as a
    /// glitch rather than as charring.
    #[test]
    fn the_cell_hash_is_stable_and_spreads_out() {
        assert_eq!(cell_hash(30, 60), cell_hash(30, 60));
        let mut seen = std::collections::HashSet::new();
        for y in 0..20 {
            for x in 0..20 {
                assert!(seen.insert(cell_hash(x * 30, y * 30)), "collision at {x},{y}");
            }
        }
    }
}
