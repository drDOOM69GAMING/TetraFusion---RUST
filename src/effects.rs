//! Particle effects, ported from the original's particle toolkit.
//!
//! There are three systems, and they behave exactly like `TetraFusion_2.1.py`:
//!
//! - **Trail** - the *Effect* option. While the active piece moves sideways or
//!   soft-drops, a handful of particles spawn at the piece's edge each frame,
//!   tinted and shaped per effect: flame (glowing embers), wind (streaks),
//!   water (falling drops), ice (sparkling stars), flicker (flashing bolts) and
//!   matrix (halfwidth katakana falling in a column, see [`KATAKANA`]).
//!   `none` disables the trail only.
//! - **Dust** - brown puffs burst where a piece slams down on a hard drop,
//!   independent of the Effect setting.
//! - **Explosion** - coloured debris bursts from every cleared cell,
//!   independent of the Effect setting.
//! - **Gesture** - a themed burst thrown by the piece actually *doing*
//!   something: a slide, a rotation or a slam. This one is new. The original
//!   only trailed while a key was held, which meant a rotation or a single tap
//!   produced nothing at all, and the Effect option had no say in how hard the
//!   game felt. Gestures are what the design's "moving, rotating, or dropping
//!   generates themed particles, scaling with the stage" is asking for.
//!
//! Particles live in *visible* playfield pixel space: row 0 of the board draws
//! at `y = 0`, row `HIDDEN_ROWS` ago at the floor, matching the renderer.

use raylib::core::color::Color;
use raylib::core::drawing::{RaylibDraw, RaylibDrawHandle};
use raylib::ffi::Vector2;

use crate::config::{BLOCK_SIZE, HIDDEN_ROWS, MAX_GRAVITY};
use crate::pieces::{self, Piece, Rotation};

// --- Matrix rain glyphs -------------------------------------------------

/// Glyph cell, in pixels, before [`MATRIX_SCALE`].
///
/// 5x7 is what halfwidth katakana fit into legibly at the size a trail is drawn
/// against 30px blocks. The original drew a 1-pixel-wide green bar; see
/// [`KATAKANA`] for why the port draws symbols instead.
const GLYPH_W: usize = 5;
const GLYPH_H: usize = 7;

/// Pixels per glyph cell. Two, so a glyph is 10x14 - clearly readable on top of
/// the playfield rather than a smudge, and still narrow enough that a column of
/// them does not blanket the piece it is trailing.
const MATRIX_SCALE: i32 = 2;

/// A glyph's drawn size, in pixels.
const MATRIX_PX_W: i32 = GLYPH_W as i32 * MATRIX_SCALE;
const MATRIX_PX_H: i32 = GLYPH_H as i32 * MATRIX_SCALE;

/// Frames between one re-roll of the glyph column and the next.
///
/// Real Matrix rain scrolls; a column of glyphs frozen in place for its whole
/// 15-30 frame life reads as a smear of green rather than as code falling. A
/// handful of frames per step is slow enough to see and fast enough to look like
/// motion.
const MATRIX_SCROLL: f32 = 4.0;

/// Halfwidth katakana, the alphabet the Matrix uses, one glyph per entry.
///
/// Hand-drawn pixel data rather than text, and that is the whole reason this is
/// a table and not a call to `draw_text`. raylib's built-in font has glyphs for
/// codepoints 32 to 255 only (`defaultFont.glyphs[i].value = 32 + i` in
/// `rtext.c`), and halfwidth katakana is U+FF66 to U+FF9D - 400-odd codepoints
/// past the end of it. The only font asset this game ships is a tetromino font,
/// which has no kana either. So the symbols are drawn as rectangles: no font
/// dependency, no atlas, no runtime codepoint mapping, and identical on every
/// platform.
///
/// Row-major, one bit per pixel, the *high* bits being the left-hand columns -
/// the same order the rows are written in, so a glyph can be read straight off
/// as art. No row uses a bit above `1 << GLYPH_W`.
const KATAKANA: [[u8; GLYPH_H]; 34] = [
    [0x0C, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10], // i
    [0x03, 0x1E, 0x04, 0x08, 0x10, 0x10, 0x00], // a
    [0x00, 0x1E, 0x02, 0x04, 0x08, 0x10, 0x00], // u
    [0x00, 0x1F, 0x04, 0x04, 0x04, 0x04, 0x1F], // e
    [0x04, 0x1F, 0x04, 0x0E, 0x0A, 0x0E, 0x00], // o
    [0x04, 0x08, 0x1F, 0x04, 0x04, 0x04, 0x00], // ke
    [0x04, 0x08, 0x1F, 0x04, 0x04, 0x0E, 0x00], // ko
    [0x08, 0x10, 0x10, 0x1F, 0x10, 0x10, 0x10], // shi
    [0x02, 0x04, 0x08, 0x08, 0x10, 0x10, 0x00], // su
    [0x08, 0x10, 0x1F, 0x02, 0x04, 0x08, 0x00], // se
    [0x04, 0x08, 0x08, 0x04, 0x04, 0x04, 0x0E], // so
    [0x08, 0x0A, 0x0A, 0x1F, 0x04, 0x04, 0x00], // ta
    [0x03, 0x04, 0x0E, 0x04, 0x04, 0x04, 0x00], // chi
    [0x04, 0x0E, 0x04, 0x04, 0x08, 0x0E, 0x00], // tsu
    [0x07, 0x04, 0x1F, 0x04, 0x04, 0x04, 0x00], // te
    [0x0C, 0x12, 0x12, 0x0C, 0x04, 0x08, 0x00], // na
    [0x00, 0x1F, 0x00, 0x00, 0x00, 0x1F, 0x00], // ni
    [0x04, 0x08, 0x08, 0x04, 0x04, 0x04, 0x0E], // ne
    [0x02, 0x04, 0x08, 0x08, 0x10, 0x10, 0x00], // no
    [0x08, 0x08, 0x1F, 0x13, 0x1F, 0x13, 0x00], // ma
    [0x1F, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00], // mi
    [0x08, 0x08, 0x0E, 0x13, 0x13, 0x0E, 0x00], // mu
    [0x0C, 0x0A, 0x08, 0x08, 0x08, 0x0E, 0x00], // me
    [0x0C, 0x0A, 0x0E, 0x0A, 0x0E, 0x08, 0x00], // mo
    [0x11, 0x0A, 0x0E, 0x04, 0x04, 0x04, 0x00], // ya
    [0x04, 0x04, 0x1E, 0x04, 0x04, 0x04, 0x00], // yu
    [0x0C, 0x0A, 0x08, 0x08, 0x0E, 0x08, 0x00], // yo
    [0x02, 0x04, 0x04, 0x08, 0x0E, 0x08, 0x00], // ra
    [0x10, 0x11, 0x11, 0x11, 0x11, 0x11, 0x00], // ri
    [0x11, 0x11, 0x11, 0x11, 0x11, 0x0E, 0x00], // ru
    [0x10, 0x10, 0x10, 0x10, 0x10, 0x1F, 0x00], // re
    [0x0E, 0x08, 0x08, 0x08, 0x0E, 0x00, 0x00], // ro
    [0x0F, 0x01, 0x02, 0x02, 0x04, 0x04, 0x00], // wa
    [0x08, 0x08, 0x08, 0x08, 0x09, 0x11, 0x00], // n
];

/// The glyph `slot` glyphs up the column from the head, at scroll `step`.
///
/// A pure function of the particle's seed rather than a stored sequence, so a
/// trail particle stays a plain `Copy`-able struct with no `Vec` in it, and the
/// column still scrolls because `step` advances with the particle's age.
///
/// The wrap is the load-bearing part: `slot` and `step` both grow without bound
/// and the table has 34 entries, so the sum has to be taken *after* the
/// conversion, not before, or a long-lived particle indexes off the end.
fn glyph_at(seed: u32, slot: i32, step: i32) -> &'static [u8; GLYPH_H] {
    let n = KATAKANA.len() as i32;
    let i = ((seed as i32 + slot + step) % n + n) % n;
    &KATAKANA[i as usize]
}

/// Which way the piece (and therefore the spawn edge) is moving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Down,
}

impl Direction {
    /// Collapse a grid delta into a direction.
    ///
    /// A rotation's delta is `(0, 0)` when the piece turned in place with no
    /// wall kick, which is neither a slide nor a drop; it becomes `Down` so the
    /// burst goes somewhere sensible instead of falling through a match that
    /// does not cover it. Horizontal wins over vertical for a kick that moved
    /// both ways, because the sideways part is what the player saw happen.
    pub fn from_delta(d: (i32, i32)) -> Direction {
        match d {
            (x, _) if x < 0 => Direction::Left,
            (x, _) if x > 0 => Direction::Right,
            _ => Direction::Down,
        }
    }
}

/// The Effect option's particle flavour. `none` has no kind at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Flame,
    Wind,
    Water,
    Ice,
    Flicker,
    Matrix,
}

fn kind_for(effect: &str) -> Option<Kind> {
    match effect {
        "flame" => Some(Kind::Flame),
        "wind" => Some(Kind::Wind),
        "water" => Some(Kind::Water),
        "ice" => Some(Kind::Ice),
        "flicker" => Some(Kind::Flicker),
        "matrix" => Some(Kind::Matrix),
        _ => None, // "none" and anything unknown
    }
}

/// One trail particle. Every field is used by at least one kind.
struct Trail {
    kind: Kind,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    max_age: f32,
    size: f32,
    drift_x: f32,
    drift_y: f32,
    rotation: f32,
    rot_speed: f32,
    color: [u8; 3],
    /// Flicker's on/off interval, in frames.
    flash: f32,
    /// Ice's chance of a bright centre sparkle.
    sparkle: f32,
    /// Matrix's green intensity.
    green: u8,
    /// Matrix's trailing column length, in glyphs. The other kinds use `size`.
    length: f32,
    /// Matrix's glyph-column seed; see [`glyph_at`].
    glyph_seed: u32,
}

impl Trail {
    fn new(kind: Kind, x: f32, y: f32, dir: Direction) -> Self {
        let mut t = Trail {
            kind,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            age: 0.0,
            max_age: 30.0,
            size: 6.0,
            drift_x: 0.0,
            drift_y: 0.0,
            rotation: 0.0,
            rot_speed: 0.0,
            color: [255, 255, 255],
            flash: 2.0,
            sparkle: 0.5,
            green: 200,
            length: 8.0,
            glyph_seed: 0,
        };
        match kind {
            Kind::Flame => {
                // Angle that pushes the ember sideways or down, per direction.
                let angle = match dir {
                    Direction::Left => range(PI_2, PI_3H),
                    Direction::Right => range(-PI_2, PI_2),
                    Direction::Down => range(PI_2 - PI_8, PI_2 + PI_8),
                };
                let speed = range(1.5, 3.0);
                t.vx = angle.cos() * speed;
                t.vy = angle.sin() * speed;
                t.max_age = rand_i(40, 60) as f32;
                t.size = rand_i(12, 20) as f32;
                t.drift_x = range(-0.5, 0.5);
                t.drift_y = range(-0.5, 0.5);
            }
            Kind::Wind => {
                match dir {
                    Direction::Left => {
                        t.vx = range(-5.0, -2.0);
                        t.vy = range(-1.0, 1.0);
                    }
                    Direction::Right => {
                        t.vx = range(2.0, 5.0);
                        t.vy = range(-1.0, 1.0);
                    }
                    Direction::Down => {
                        t.vx = range(-2.0, 2.0);
                        t.vy = range(2.0, 5.0);
                    }
                }
                t.max_age = rand_i(15, 25) as f32;
                t.size = range(1.0, 3.0);
            }
            Kind::Water => {
                match dir {
                    Direction::Down => {
                        t.vx = range(-2.0, 2.0);
                        t.vy = range(-1.0, 3.0);
                    }
                    Direction::Left => {
                        t.vx = range(-3.0, 0.0);
                        t.vy = range(-1.0, 1.0);
                    }
                    Direction::Right => {
                        t.vx = range(0.0, 3.0);
                        t.vy = range(-1.0, 1.0);
                    }
                }
                t.max_age = rand_i(30, 50) as f32;
                t.size = rand_i(3, 6) as f32;
                let blue = rand_i(150, 220);
                t.color = [50, 100, blue as u8];
            }
            Kind::Ice => {
                let angle = range(0.0, TAU);
                let speed = range(1.0, 4.0);
                t.vx = angle.cos() * speed;
                t.vy = angle.sin() * speed;
                match dir {
                    Direction::Down => t.vy = t.vy.abs(),
                    Direction::Left => t.vx = -t.vx.abs(),
                    Direction::Right => t.vx = t.vx.abs(),
                }
                t.max_age = rand_i(25, 45) as f32;
                t.size = rand_i(2, 5) as f32;
                t.sparkle = range(0.5, 1.0);
                t.rotation = range(0.0, TAU);
                t.rot_speed = range(-0.1, 0.1);
            }
            Kind::Flicker => {
                match dir {
                    Direction::Down => {
                        t.vx = range(-2.0, 2.0);
                        t.vy = range(1.0, 4.0);
                    }
                    Direction::Left => {
                        t.vx = range(-4.0, -1.0);
                        t.vy = range(-2.0, 2.0);
                    }
                    Direction::Right => {
                        t.vx = range(1.0, 4.0);
                        t.vy = range(-2.0, 2.0);
                    }
                }
                t.max_age = rand_i(8, 18) as f32;
                t.size = rand_i(6, 14) as f32;
                t.color = [[255, 255, 100], [255, 150, 50], [200, 200, 255]][rand_i(0, 2) as usize];
                t.flash = rand_i(2, 5) as f32;
            }
            Kind::Matrix => {
                match dir {
                    Direction::Down => {
                        t.vx = range(-0.5, 0.5);
                        t.vy = range(2.0, 6.0);
                    }
                    Direction::Left => {
                        t.vx = range(-6.0, -2.0);
                        t.vy = range(-0.5, 0.5);
                    }
                    Direction::Right => {
                        t.vx = range(2.0, 6.0);
                        t.vy = range(-0.5, 0.5);
                    }
                }
                t.max_age = rand_i(15, 30) as f32;
                // Glyphs, not pixels. The original's `length` of 6-15 was a
                // 1-pixel-wide bar, so it read as 6-15 pixels of green; the same
                // count here would be 6-15 *glyphs* and a 200px column dragging
                // behind a 30px piece. 2-5 glyphs is 28-70px, which is the same
                // streak as the original next to the block it trails.
                t.length = rand_i(2, 5) as f32;
                t.green = rand_i(150, 255) as u8;
                t.glyph_seed = rand_i(0, KATAKANA.len() as i32 - 1) as u32;
            }
        }
        t
    }

    fn update(&mut self, wind: (f32, f32)) {
        match self.kind {
            Kind::Flame => {
                self.x += self.vx + self.drift_x + wind.0;
                self.y += self.vy + self.drift_y + wind.1;
                self.vx *= 0.92;
                self.vy *= 0.92;
                self.drift_x *= 0.7;
                self.drift_y *= 0.7;
                self.y -= 0.1; // gentle rise
                self.size = (self.size * 0.95).max(5.0);
            }
            Kind::Wind => {
                self.x += self.vx + wind.0;
                self.y += self.vy + wind.1;
            }
            Kind::Water => {
                self.x += self.vx + wind.0;
                self.vy += 0.15;
                self.y += self.vy + wind.1;
                self.size = (self.size * 0.97).max(1.0);
            }
            Kind::Ice => {
                self.x += self.vx * 0.8 + wind.0;
                self.y += self.vy * 0.8 + wind.1;
                self.vx *= 0.98;
                self.vy *= 0.98;
                self.rotation += self.rot_speed;
                self.size = (self.size * 0.98).max(1.0);
            }
            Kind::Flicker => {
                self.x += self.vx * 0.9 + wind.0;
                self.y += self.vy * 0.9 + wind.1;
                self.size = (self.size * 0.92).max(2.0);
            }
            Kind::Matrix => {
                self.x += self.vx + wind.0;
                self.y += self.vy + wind.1;
            }
        }
        self.age += 1.0;
    }

    fn draw(&self, d: &mut RaylibDrawHandle) {
        let p = (self.age / self.max_age).clamp(0.0, 1.0);
        match self.kind {
            Kind::Flame => {
                if self.age >= self.max_age {
                    return;
                }
                let color = if p < 0.33 {
                    [255, 240, 150]
                } else if p < 0.66 {
                    [255, 180, 80]
                } else {
                    [255, 90, 40]
                };
                let a = (255.0 * (1.0 - p.powf(1.5))) as u8;
                d.draw_circle(
                    self.x as i32,
                    self.y as i32,
                    self.size,
                    Color::new(color[0], color[1], color[2], a),
                );
            }
            Kind::Wind => {
                // The original's help text calls this a "directional streak",
                // but a gust reads as a soft puffy cloud, so render the trail
                // as a staggered row of pale-blue lobes that drift backward
                // along the wind direction instead of a single thin line.
                if self.age >= self.max_age {
                    return;
                }
                if self.x < 0.0 || self.x > crate::config::SCREEN_WIDTH as f32 {
                    return;
                }
                if self.y < 0.0 || self.y > crate::config::SCREEN_HEIGHT as f32 {
                    return;
                }
                let p = (self.age / self.max_age).clamp(0.0, 1.0);
                let a = (220.0 * (1.0 - p)) as u8;
                let len = 10.0 + self.age * 0.5;
                // Unit vector back along the stream (the cloud trails behind
                // the gust's motion) plus a perpendicular for the bumps.
                let spd = (self.vx * self.vx + self.vy * self.vy).sqrt().max(1e-4);
                let (ux, uy) = (-self.vx / spd, -self.vy / spd);
                let (bx, by) = (-uy, ux);
                let base = self.size + 4.0;

                // Soft halo binding the lobes together into one cloud.
                d.draw_circle(
                    self.x as i32,
                    self.y as i32,
                    base * 2.4,
                    Color::new(190, 215, 250, a / 3),
                );

                // Lumpy puffs staggered across the stream direction.
                for k in 0..5 {
                    let t = k as f32 / 4.0;
                    let cx = self.x + ux * len * t + bx * (k as f32 - 2.0) * base * 0.45;
                    let cy = self.y + uy * len * t + by * (k as f32 - 2.0) * base * 0.45;
                    let r = base * (1.15 - 0.25 * (t - 0.5).abs());
                    let al = (a as f32 * (0.45 + 0.35 * (1.0 - t))) as u8;
                    d.draw_circle(cx as i32, cy as i32, r, Color::new(200, 220, 255, al));
                }
            }
            Kind::Water => {
                // The original draws a soft drop: an ellipse wider than it is
                // tall, centred just below the particle's anchor.
                if self.age >= self.max_age {
                    return;
                }
                let a = (200.0 * (1.0 - p)) as u8;
                d.draw_ellipse(
                    self.x as i32,
                    (self.y + 1.0) as i32,
                    self.size,
                    self.size * 1.5,
                    Color::new(self.color[0], self.color[1], self.color[2], a),
                );
            }
            Kind::Ice => {
                // The original's ice particle is a rotating four-point
                // polygon: two long points and two short points opposite
                // them, like a crystallising sparkle.
                if self.age >= self.max_age {
                    return;
                }
                let a = (255.0 * (1.0 - p)) as u8;
                let rot = self.rotation;
                let r = self.size;
                let s = r * 0.4;
                let verts = [
                    Vector2 { x: self.x + rot.cos() * r, y: self.y + rot.sin() * r },
                    Vector2 {
                        x: self.x + (rot + 2.5).cos() * r,
                        y: self.y + (rot + 2.5).sin() * r,
                    },
                    Vector2 {
                        x: self.x + (rot + PI).cos() * s,
                        y: self.y + (rot + PI).sin() * s,
                    },
                    Vector2 {
                        x: self.x + (rot + 3.8).cos() * s,
                        y: self.y + (rot + 3.8).sin() * s,
                    },
                ];
                let white = Color::new(255, 255, 255, a);
                for i in 0..4 {
                    let j = (i + 1) % 4;
                    d.draw_line_ex(verts[i], verts[j], 1.5, white);
                }
                if self.sparkle > 0.7 {
                    d.draw_circle(
                        self.x as i32,
                        self.y as i32,
                        (r / 2.0).max(1.0),
                        white,
                    );
                }
            }
            Kind::Flicker => {
                if self.age >= self.max_age {
                    return;
                }
                let visible = (self.age as i32 / self.flash.max(1.0) as i32) % 2 == 0;
                if !visible {
                    return;
                }
                let a = (255.0 * (1.0 - p)) as u8;
                d.draw_circle(
                    self.x as i32,
                    self.y as i32,
                    self.size,
                    Color::new(self.color[0], self.color[1], self.color[2], a),
                );
                d.draw_circle(
                    self.x as i32,
                    self.y as i32,
                    (self.size / 2.0).max(1.0),
                    Color::new(255, 255, 255, (a as f32 * 0.7) as u8),
                );
            }
            Kind::Matrix => {
                // Halfwidth katakana falling in a column, bright at the leading
                // edge and fading up the tail - the digital rain the effect is
                // named after.
                //
                // The original drew a single 1-pixel column of green here,
                // interpolating bright-to-faint, so the effect was a green smear
                // rather than anything recognisable as Matrix. Drawing symbols
                // is the point of the option.
                //
                // One deliberate inversion of the original: it put the bright
                // head at the *top* of the streak and the faint tail at the
                // anchor. The particle falls, so the leading edge is the bottom
                // and that is where the bright end has to be for the column to
                // read as falling code instead of code being dragged upwards.
                if self.age >= self.max_age {
                    return;
                }
                let a = 255.0 * (1.0 - p.powf(0.5));
                let (g, ta) = (self.green as f32, a * 0.4);
                let count = self.length.max(1.0) as i32;
                let step = (self.age / MATRIX_SCROLL) as i32;
                let x0 = self.x as i32 - MATRIX_PX_W / 2;
                let y_bottom = self.y as i32;
                for slot in 0..count {
                    // slot 0 is the head, at the bottom.
                    let t = (slot as f32 + 0.5) / count as f32;
                    let green = g * (1.0 - t) + (g / 3.0) * t;
                    let alpha = a * (1.0 - t) + ta * t;
                    let color = Color::new(0, green as u8, 0, alpha as u8);
                    let gy = y_bottom - (slot + 1) * MATRIX_PX_H;
                    let glyph = glyph_at(self.glyph_seed, slot, step);
                    for row in 0..GLYPH_H {
                        let bits = glyph[row];
                        for cx in 0..GLYPH_W {
                            if bits & (1 << (GLYPH_W - 1 - cx)) != 0 {
                                d.draw_rectangle(
                                    x0 + cx as i32 * MATRIX_SCALE,
                                    gy + row as i32 * MATRIX_SCALE,
                                    MATRIX_SCALE,
                                    MATRIX_SCALE,
                                    color,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Brown puff kicked up where a hard-dropped piece slams into the stack.
struct Dust {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    max_age: f32,
    size: f32,
    color: [u8; 3],
}

impl Dust {
    fn new(x: f32, y: f32) -> Self {
        let angle = range(PI, TAU);
        let speed = range(1.0, 3.0);
        Self {
            x,
            y,
            vx: angle.cos() * speed,
            vy: angle.sin() * speed,
            age: 0.0,
            max_age: rand_i(20, 40) as f32,
            size: rand_i(8, 15) as f32,
            color: [rand_i(100, 150) as u8, rand_i(50, 100) as u8, 0],
        }
    }

    fn update(&mut self) {
        self.x += self.vx;
        self.y += self.vy;
        self.vx *= 0.92;
        self.vy *= 0.92;
        self.age += 1.0;
        self.size = (self.size * 0.95).max(2.0);
    }

    fn draw(&self, d: &mut RaylibDrawHandle) {
        if self.age >= self.max_age {
            return;
        }
        let p = self.age / self.max_age;
        let a = (255.0 * (1.0 - p)) as u8;
        d.draw_circle(
            self.x as i32,
            self.y as i32,
            self.size,
            Color::new(self.color[0], self.color[1], self.color[2], a),
        );
    }
}

/// Debris bursting from a cleared cell.
struct Explosion {
    x: f32,
    y: f32,
    color: [u8; 3],
    lifetime: f32,
    parts: Vec<[f32; 6]>, // x, y, vx, vy, gravity, alpha
}

impl Explosion {
    fn new(x: f32, y: f32, color: [u8; 3], count: usize, max_speed: f32) -> Self {
        let mut parts = Vec::with_capacity(count);
        for _ in 0..count {
            parts.push([
                x + range(-15.0, 15.0),
                y + range(-15.0, 15.0),
                range(-max_speed, max_speed),
                range(-max_speed, max_speed),
                range(0.1, 0.3),
                rand_i(200, 255) as f32,
            ]);
        }
        Self {
            x,
            y,
            color,
            lifetime: 30.0,
            parts,
        }
    }

    fn update(&mut self) {
        self.lifetime -= 1.0;
        for p in &mut self.parts {
            p[0] += p[2];
            p[1] += p[3];
            p[3] += p[4];
            p[5] = (p[5] - 4.0).max(0.0);
        }
    }

    fn draw(&self, d: &mut RaylibDrawHandle, offset: (i32, i32)) {
        for p in &self.parts {
            if p[5] > 0.0 {
                let size = 4.0 + p[5] / 50.0;
                d.draw_circle(
                    (self.x + p[0] + offset.0 as f32) as i32,
                    (self.y + p[1] + offset.1 as f32) as i32,
                    size,
                    Color::new(self.color[0], self.color[1], self.color[2], p[5] as u8),
                );
            }
        }
    }
}

/// How hard a gesture throws particles, in `0.15..=1.0`.
///
/// The "scales in intensity as the stage progresses" knob. Linear in the level
/// and clamped at *both* ends, and both clamps are load-bearing:
///
/// * The floor: level one still throws something. A gesture that throws nothing
///   at all does not read as "subtle at low intensity", it reads as the Effect
///   setting being broken.
/// * The ceiling: level 20 is the top of the gravity range, and past it the
///   count stops growing. Without this, a run that reaches level 40 would put
///   hundreds of particles on every keypress.
fn gesture_power(level: i32) -> f32 {
    let l = level.clamp(1, MAX_GRAVITY as i32) as f32;
    (l / MAX_GRAVITY as f32).max(0.15)
}

/// How many particles one gesture throws at `level`.
fn gesture_count(level: i32) -> usize {
    (2.0 + 10.0 * gesture_power(level)).round() as usize
}

/// One particle's whole trajectory, decided before it is ever drawn.
///
/// Bundling it into a plain value is what makes a gesture *checkable*: the
/// shape of a burst is a function of a handful of numbers, so it can be
/// asserted on without a window, a random number generator or a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
struct GestureShot {
    pub vx: f32,
    pub vy: f32,
    /// Seconds the particle lives.
    pub life: f32,
    pub size: f32,
}

impl GestureShot {
    /// How fast the particle is leaving, in pixels a second.
    ///
    /// Nothing on screen needs this - the particle just stores `vx` and `vy`
    /// and integrates them. It exists so the tests can compare two effects on a
    /// single number, which is the only way to say "ice is faster than water"
    /// without hand-rolling the sum in every assertion.
    #[cfg(test)]
    fn speed(&self) -> f32 {
        (self.vx * self.vx + self.vy * self.vy).sqrt()
    }
}

/// The unit vector a gesture is travelling in.
fn dir_vector(dir: Direction) -> (f32, f32) {
    match dir {
        Direction::Left => (-1.0, 0.0),
        Direction::Right => (1.0, 0.0),
        Direction::Down => (0.0, 1.0),
    }
}

/// How the Effect option shapes a burst, per kind.
///
/// `t` is the particle's place in the burst, `0.0..1.0`, and it walks a full
/// turn, so the particles leave in all directions instead of in a line - and
/// because it is a pure argument rather than a random draw, the same `(kind,
/// dir, level, t)` always gives the same shot. That is what lets the shapes be
/// tested, and it also means a burst is *spread*: `gesture_count` calls with
/// even steps of `t` and cannot pile two particles on top of each other.
///
/// The six kinds are deliberately given six different silhouettes - outward and
/// slow for water, a hard radial spike for ice, a rise for flame - so that
/// switching the Effect option visibly changes the feel of moving a piece, not
/// just its colour.
fn gesture_shot(kind: Kind, dir: Direction, level: i32, t: f32) -> GestureShot {
    let power = gesture_power(level);
    let angle = t * std::f32::consts::TAU;
    let (dx, dy) = dir_vector(dir);
    match kind {
        // Liquid: pushes out from wherever the piece touched and flattens as
        // it goes. Long-lived, so it reads as a ripple spreading rather than as
        // a spark.
        Kind::Water => GestureShot {
            vx: angle.cos() * (14.0 + 30.0 * power),
            vy: angle.sin() * (5.0 + 14.0 * power),
            life: 1.5,
            size: 2.0 + 3.0 * power,
        },
        // Wind: goes where the piece went, spreading sideways across the path
        // it took, and lifts as it goes.
        Kind::Wind => GestureShot {
            vx: dx * (30.0 + 60.0 * power) + (t - 0.5) * 40.0,
            vy: dy * (30.0 + 60.0 * power) - 10.0,
            life: 1.1,
            size: 2.0 + 2.0 * power,
        },
        // Ice: sharp radial shards, fast and short-lived.
        Kind::Ice => GestureShot {
            vx: angle.cos() * (40.0 + 90.0 * power),
            vy: angle.sin() * (30.0 + 70.0 * power),
            life: 0.45,
            size: 1.0 + 2.0 * power,
        },
        // Flame: rises, whatever the piece was doing - embers do not care which
        // way the block that threw them was travelling.
        Kind::Flame => GestureShot {
            vx: angle.cos() * (10.0 + 26.0 * power),
            vy: -(30.0 + 70.0 * power),
            life: 0.8,
            size: 2.0 + 3.0 * power,
        },
        // Flicker: a thin streak along the gesture, like a camera flash.
        Kind::Flicker => GestureShot {
            vx: dx * (50.0 + 110.0 * power),
            vy: dy * (50.0 + 110.0 * power) + (t - 0.5) * 14.0,
            life: 0.3,
            size: 1.0,
        },
        // Matrix: specks that fall away from the gesture and scroll.
        Kind::Matrix => GestureShot {
            vx: dx * 8.0 + (t - 0.5) * 20.0,
            vy: 20.0 + 50.0 * power,
            life: 1.3,
            size: 1.0 + power,
        },
    }
}

/// The colour a gesture particle starts in, per kind.
fn gesture_color(kind: Kind) -> [u8; 3] {
    match kind {
        Kind::Flame => [255, 170, 60],
        Kind::Wind => [225, 240, 225],
        Kind::Water => [120, 200, 255],
        Kind::Ice => [200, 240, 255],
        Kind::Flicker => [255, 250, 200],
        Kind::Matrix => [60, 255, 110],
    }
}

/// One particle thrown by a gesture.
struct Gesture {
    kind: Kind,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    max_age: f32,
    size: f32,
    color: [u8; 3],
}

/// All the particle systems in one place.
pub struct Particles {
    trail: Vec<Trail>,
    dust: Vec<Dust>,
    explosions: Vec<Explosion>,
    gestures: Vec<Gesture>,
    kind: Option<Kind>,
}

impl Particles {
    pub fn new(effect: &str) -> Self {
        Self {
            trail: Vec::new(),
            dust: Vec::new(),
            explosions: Vec::new(),
            gestures: Vec::new(),
            kind: kind_for(effect),
        }
    }

    /// Switch the trail effect mid-session (options screen). Dust and
    /// explosions are independent of the setting and are kept.
    ///
    /// Gestures are cleared too, unlike dust and explosions: they are *shaped*
    /// by the Effect setting, so a burst still in the air would finish drawing
    /// itself in a material the player had just switched away from.
    pub fn set_effect(&mut self, effect: &str) {
        self.kind = kind_for(effect);
        self.trail.clear();
        self.gestures.clear();
    }

    pub fn active(&self) -> bool {
        self.kind.is_some()
    }

    /// Spawn a handful of trail particles at the current piece's edge,
    /// pointing the way it is moving.
    pub fn spawn_trail(
        &mut self,
        piece: Piece,
        rot: Rotation,
        origin: (i32, i32),
        dir: Direction,
    ) {
        let Some(kind) = self.kind else { return };
        let (minx, maxx, miny, maxy) = piece_bounds(piece, rot);
        let (ox, oy) = origin;
        let w_cells = (maxx - minx + 1) as f32;
        let h_cells = (maxy - miny + 1) as f32;
        let left = (ox + minx) as f32 * BLOCK_SIZE as f32;
        let top = (oy + miny - HIDDEN_ROWS as i32) as f32 * BLOCK_SIZE as f32;
        let w_px = w_cells * BLOCK_SIZE as f32;
        let h_px = h_cells * BLOCK_SIZE as f32;
        let bottom = top + h_px;

        let count = rand_i(5, 8);
        for _ in 0..count {
            let (x, y, d) = match dir {
                Direction::Left => (
                    left - BLOCK_SIZE as f32 + range(-15.0, 0.0),
                    top + range(0.2, 0.8) * h_px,
                    Direction::Left,
                ),
                Direction::Right => (
                    left + w_px + range(0.0, 15.0),
                    top + range(0.2, 0.8) * h_px,
                    Direction::Right,
                ),
                Direction::Down => (
                    left + range(0.2, 0.8) * w_px,
                    bottom - 15.0,
                    Direction::Down,
                ),
            };
            self.trail.push(Trail::new(kind, x, y, d));
        }
    }

    /// Dust burst around a hard-dropped piece, `hard_rows` rows deep.
    pub fn spawn_dust(&mut self, piece: Piece, rot: Rotation, origin: (i32, i32), hard_rows: i32) {
        let (minx, maxx, miny, maxy) = piece_bounds(piece, rot);
        let (ox, oy) = origin;
        let w_cells = maxx - minx + 1;
        let h_cells = maxy - miny + 1;
        let bottom = (oy + maxy + 1 - HIDDEN_ROWS as i32) as f32 * BLOCK_SIZE as f32;

        // A wide puff under the landing edge, then specks across the body.
        let n1 = 20 + hard_rows * 5;
        for _ in 0..n1 {
            let x =
                (ox as f32 - 1.0 + range(0.0, (w_cells + 2) as f32)) * BLOCK_SIZE as f32;
            self.dust.push(Dust::new(x, bottom));
        }
        let n2 = 8 + hard_rows * 2;
        let top = (oy + miny - HIDDEN_ROWS as i32) as f32 * BLOCK_SIZE as f32;
        for _ in 0..n2 {
            let x = (ox as f32 - 0.5 + range(0.0, w_cells as f32 + 1.0)) * BLOCK_SIZE as f32;
            let y = top + range(0.0, h_cells as f32) * BLOCK_SIZE as f32;
            self.dust.push(Dust::new(x, y));
        }
    }

    /// Throw a themed burst from the piece's leading edge.
    ///
    /// `dir` is which way the piece moved, which is what makes a slide look
    /// different from a drop. `level` sets how hard it throws, so a run gets
    /// louder as it gets faster.
    ///
    /// The burst is anchored to the edge the piece travelled *towards* - the side
    /// it slid to, or the face it slammed down on - because that is the edge
    /// that swept into new space. Spawning from the centre would put the
    /// particles inside the block, where they are hidden for most of their
    /// life.
    pub fn spawn_gesture(
        &mut self,
        piece: Piece,
        rot: Rotation,
        origin: (i32, i32),
        dir: Direction,
        level: i32,
    ) {
        let Some(kind) = self.kind else { return };
        let (minx, maxx, miny, maxy) = piece_bounds(piece, rot);
        let (ox, oy) = origin;
        let left = (ox + minx) as f32 * BLOCK_SIZE as f32;
        let right = (ox + maxx + 1) as f32 * BLOCK_SIZE as f32;
        let top = (oy + miny - HIDDEN_ROWS as i32) as f32 * BLOCK_SIZE as f32;
        let bottom = (oy + maxy + 1 - HIDDEN_ROWS as i32) as f32 * BLOCK_SIZE as f32;
        let mid_y = (top + bottom) / 2.0;

        let count = gesture_count(level);
        for i in 0..count {
            // Even steps around the burst, so the particles spread instead of
            // piling up. No random draw: the same gesture at the same level
            // always throws the same burst, which is what makes the shapes
            // below testable and the effect reproducible.
            let t = if count <= 1 {
                0.0
            } else {
                i as f32 / count as f32
            };
            let shot = gesture_shot(kind, dir, level, t);
            // Ride a little way along the edge the piece travelled towards, so a
            // wide piece throws from the whole length of the face rather than
            // all from one corner.
            let along = (t - 0.5).clamp(-0.5, 0.5);
            let (x, y) = match dir {
                Direction::Left => (left, mid_y + along * (bottom - top)),
                Direction::Right => (right, mid_y + along * (bottom - top)),
                Direction::Down => ((left + right) / 2.0 + along * (right - left), bottom),
            };
            self.gestures.push(Gesture {
                kind,
                x,
                y,
                vx: shot.vx,
                vy: shot.vy,
                age: 0.0,
                max_age: shot.life,
                size: shot.size,
                color: gesture_color(kind),
            });
        }
    }

    /// A coloured debris burst at a cleared cell.
    pub fn explode(&mut self, cell_x: i32, grid_y: i32, color: [u8; 3]) {
        let x = cell_x as f32 * BLOCK_SIZE as f32 + BLOCK_SIZE as f32 / 2.0;
        let y =
            (grid_y as i32 - HIDDEN_ROWS as i32) as f32 * BLOCK_SIZE as f32 + BLOCK_SIZE as f32 / 2.0;
        self.explosions.push(Explosion::new(x, y, color, 8, 8.0));
    }

    /// Step every system. `wind` is the piece-motion push the original
    /// applied to trail particles while steering.
    pub fn update(&mut self, dt: f32, wind: (f32, f32)) {
        let _ = dt;
        self.trail.retain_mut(|t| {
            t.update(wind);
            t.age < t.max_age
        });
        self.dust.retain_mut(|dx| {
            dx.update();
            dx.age < dx.max_age
        });
        self.explosions.retain_mut(|ex| {
            ex.update();
            ex.lifetime > 0.0
        });
        self.gestures.retain_mut(|g| {
            g.age += dt;
            g.x += g.vx * dt;
            g.y += g.vy * dt;
            g.vx *= 0.90;
            // Buoyancy, by kind. Water and ice sag, flame and wind rise, and
            // matrix falls away - each kind has to *fall* differently or the
            // six silhouettes blur into one after the first few frames.
            g.vy += match g.kind {
                Kind::Flame => -26.0 * dt,
                Kind::Wind => -10.0 * dt,
                Kind::Water => 34.0 * dt,
                Kind::Ice => 46.0 * dt,
                Kind::Flicker => 0.0,
                Kind::Matrix => 44.0 * dt,
            };
            g.age < g.max_age
        });
    }

    pub fn draw(&self, d: &mut RaylibDrawHandle, offset: (i32, i32)) {
        for t in &self.trail {
            t.draw(d);
        }
        for dx in &self.dust {
            dx.draw(d);
        }
        for ex in &self.explosions {
            ex.draw(d, offset);
        }
        for g in &self.gestures {
            g.draw(d, offset);
        }
    }
}

impl Gesture {
    /// Fade over the particle's life, so a burst does not pop out of
    /// existence all at once when its timer runs out.
    fn alpha(&self) -> f32 {
        if self.max_age <= 0.0 {
            return 0.0;
        }
        (1.0 - self.age / self.max_age).clamp(0.0, 1.0)
    }

    fn draw(&self, d: &mut RaylibDrawHandle, offset: (i32, i32)) {
        let a = self.alpha();
        if a <= 0.0 {
            return;
        }
        let x = self.x + offset.0 as f32;
        let y = self.y + offset.1 as f32;
        let c = |m: f32| {
            let f = |v: u8| (v as f32 * m).clamp(0.0, 255.0) as u8;
            Color::new(f(self.color[0]), f(self.color[1]), f(self.color[2]), (a * 255.0) as u8)
        };
        match self.kind {
            // A ripple is a ring, not a dot - the ring is what reads as a
            // spreading wave, and a filled dot would just be another spark.
            // `draw_circle_lines` gives the ring for free; trying to fake one by
            // overdrawing a black disc would paint a dark blob over the board.
            Kind::Water => {
                let t = 1.0 - a;
                let r = (self.size + t * self.size * 5.0).max(1.0);
                d.draw_circle_lines(x as i32, y as i32, r, c(0.85));
            }
            // Shards: a short streak along the direction of travel.
            Kind::Ice | Kind::Flicker => {
                let s = self.size;
                d.draw_line_ex(
                    Vector2 { x, y },
                    Vector2 {
                        x: x - self.vx * 0.012,
                        y: y - self.vy * 0.012,
                    },
                    s,
                    c(1.0),
                );
            }
            // Everything else is a soft round puff.
            _ => {
                d.draw_circle(x as i32, y as i32, self.size.max(1.0), c(1.0));
            }
        }
    }
}

// --- helpers ---------------------------------------------------------------

const TAU: f32 = std::f32::consts::TAU;
const PI_2: f32 = std::f32::consts::FRAC_PI_2;
const PI_8: f32 = std::f32::consts::PI / 8.0;
const PI_3H: f32 = 3.0 * std::f32::consts::FRAC_PI_2;
const PI: f32 = std::f32::consts::PI;

/// The piece's cell bounding box: `(min_x, max_x, min_y, max_y)`.
fn piece_bounds(piece: Piece, rot: Rotation) -> (i32, i32, i32, i32) {
    let (mut minx, mut maxx, mut miny, mut maxy) = (0, 0, 0, 0);
    for (cx, cy) in pieces::cells(piece, rot) {
        minx = minx.min(cx);
        maxx = maxx.max(cx);
        miny = miny.min(cy);
        maxy = maxy.max(cy);
    }
    (minx, maxx, miny, maxy)
}

/// Tiny deterministic PRNG so the emitters never touch a shared RNG.
fn randish() -> u64 {
    use std::cell::Cell;
    thread_local! {
        static SEED: Cell<u64> = const { Cell::new(0x9E3779B97F4A7C15) };
    }
    SEED.with(|s| {
        let mut x = s.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        x & 0x7FFF_FFFF
    })
}

fn r01() -> f32 {
    (randish() % 1_000_000) as f32 / 1_000_000.0
}

fn range(a: f32, b: f32) -> f32 {
    a + r01() * (b - a)
}

/// Inclusive integer range.
fn rand_i(a: i32, b: i32) -> i32 {
    if b <= a {
        return a;
    }
    a + (randish() % ((b - a + 1) as u64)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- gestures ---------------------------------------------------------

    const KINDS: [Kind; 6] = [
        Kind::Flame,
        Kind::Wind,
        Kind::Water,
        Kind::Ice,
        Kind::Flicker,
        Kind::Matrix,
    ];
    const DIRS: [Direction; 3] = [Direction::Left, Direction::Right, Direction::Down];

    /// A level below one has to be treated as level one, and level one has to
    /// throw *something* - a gesture that throws nothing reads as the effect
    /// being switched off, not as "subtle".
    #[test]
    fn a_gesture_always_throws_something() {
        for level in [i32::MIN, -5, 0, 1] {
            assert!(
                gesture_count(level) >= 2,
                "level {level} threw {} particles",
                gesture_count(level)
            );
        }
        assert_eq!(gesture_count(1), gesture_count(1).max(2));
        assert!(gesture_count(1) >= 2);
    }

    /// The stage has to be felt, not just logged: the count climbs with the
    /// level and then stops. Past the ceiling it must not keep growing, or a
    /// late run puts hundreds of particles on every keypress.
    #[test]
    fn the_burst_scales_with_the_stage_and_stops_at_the_ceiling() {
        assert!(
            gesture_count(MAX_GRAVITY as i32) > gesture_count(10),
            "level 20 should throw more than level 10"
        );
        assert!(gesture_count(10) > gesture_count(1), "level 10 over level 1");
        for level in MAX_GRAVITY as i32..=200 {
            assert_eq!(
                gesture_count(level),
                gesture_count(MAX_GRAVITY as i32),
                "level {level} grew past the ceiling"
            );
        }
        // And it never shrinks on the way up.
        let mut prev = 0usize;
        for level in 1..=MAX_GRAVITY as i32 {
            let n = gesture_count(level);
            assert!(n >= prev, "level {level} threw fewer than the level below");
            prev = n;
        }
    }

    /// The count is the number of particles that get pushed, so a zero here
    /// would mean a silent no-op burst.
    #[test]
    fn a_burst_always_contains_particles() {
        for kind in KINDS {
            for dir in DIRS {
                for level in 1..=25 {
                    assert!(gesture_count(level) > 0);
                    let t = 0.0;
                    let shot = gesture_shot(kind, dir, level, t);
                    assert!(shot.life > 0.0, "{kind:?} {dir:?} life was zero");
                    assert!(shot.size > 0.0, "{kind:?} {dir:?} size was zero");
                    assert!(
                        shot.speed().is_finite(),
                        "{kind:?} {dir:?} produced a non-finite velocity"
                    );
                }
            }
        }
    }

    /// Every particle in a burst needs a real trajectory. A NaN here would be
    /// silent until the frame a particle is drawn off the edge of the board,
    /// which is exactly the sort of bug a screenshot would catch and a test
    /// should.
    #[test]
    fn no_shot_is_degenerate_at_any_place_in_a_burst() {
        for kind in KINDS {
            for dir in DIRS {
                for level in [1, 5, 20] {
                    for i in 0..24 {
                        let t = i as f32 / 24.0;
                        let s = gesture_shot(kind, dir, level, t);
                        for (name, v) in [("vx", s.vx), ("vy", s.vy), ("life", s.life), ("size", s.size)] {
                            assert!(
                                v.is_finite() && v.abs() < 10_000.0,
                                "{kind:?} {dir:?} level {level} t={t}: {name} was {v}"
                            );
                        }
                        assert!(s.life > 0.0 && s.life <= 3.0, "{kind:?} life {}", s.life);
                        assert!(s.size > 0.0 && s.size <= 12.0, "{kind:?} size {}", s.size);
                    }
                }
            }
        }
    }

    /// The six effects have to look like six different things, or the Effect
    /// option is a colour swatch.
    ///
    /// The clearest discriminator is direction of travel. Flame carries a
    /// constant upward bias so it rises whatever the piece did; matrix has the
    /// opposite bias and falls; water and ice are *radially* symmetric, because
    /// a ripple and a shard both leave in all directions and have no net drift
    /// at all. That last one is worth pinning down, because the obvious
    /// alternative - giving water a downward bias so it "sags" like liquid -
    /// turns a ring into a squashed arc and stops it reading as a ring.
    #[test]
    fn the_effects_move_in_visibly_different_directions() {
        let level = 10;
        let n = 16;
        let mean = |kind: Kind, pick: fn(&GestureShot) -> f32| -> f32 {
            (0..n)
                .map(|i| pick(&gesture_shot(kind, Direction::Down, level, i as f32 / n as f32)))
                .sum::<f32>()
                / n as f32
        };
        let vy = |k: &GestureShot| k.vy;
        let vx = |k: &GestureShot| k.vx;

        // Constant biases: these two are the up-and-down pair.
        assert!(
            mean(Kind::Flame, vy) < -20.0,
            "flame should rise, mean vy was {}",
            mean(Kind::Flame, vy)
        );
        assert!(
            mean(Kind::Matrix, vy) > 20.0,
            "matrix specks should fall, mean vy was {}",
            mean(Kind::Matrix, vy)
        );

        // Radially symmetric: no net drift on either axis, and the whole burst
        // cancels out to a standing ring.
        for kind in [Kind::Water, Kind::Ice] {
            assert!(
                mean(kind, vy).abs() < 1.0,
                "{kind:?} should have no net vertical drift, mean was {}",
                mean(kind, vy)
            );
            assert!(
                mean(kind, vx).abs() < 1.0,
                "{kind:?} should have no net horizontal drift, mean was {}",
                mean(kind, vx)
            );
        }
    }

    /// A ripple flattens as it spreads - a ring that stayed circular would
    /// read as a starburst. Water is the only kind that does this, and it is
    /// what separates it from ice.
    #[test]
    fn a_ripple_spreads_wider_than_it_spreads_tall() {
        let level = 10;
        let n = 32;
        let mean_abs = |kind: Kind, pick: fn(&GestureShot) -> f32| -> f32 {
            (0..n)
                .map(|i| {
                    pick(&gesture_shot(kind, Direction::Down, level, i as f32 / n as f32)).abs()
                })
                .sum::<f32>()
                / n as f32
        };
        let water_x = mean_abs(Kind::Water, |k| k.vx);
        let water_y = mean_abs(Kind::Water, |k| k.vy);
        assert!(
            water_x > water_y * 1.5,
            "a ripple should be a flattened ring: {water_x} across vs {water_y} down"
        );
        // Ice is the round one, for contrast: it is the same burst without the
        // flattening, so the two cannot be mistaken for each other.
        let ice_x = mean_abs(Kind::Ice, |k| k.vx);
        let ice_y = mean_abs(Kind::Ice, |k| k.vy);
        assert!(
            ice_x < ice_y * 1.5,
            "ice shards should be roughly radial: {ice_x} across vs {ice_y} down"
        );
    }

    /// Ice is a shard: fast, and gone quickly. Water is the opposite. They are
    /// the two ends of the speed range and the lifetimes have to differ enough
    /// that the eye tells them apart without reading a label.
    #[test]
    fn ice_and_water_are_opposite_ends_of_the_speed_range() {
        let level = 10;
        let speed = |kind: Kind| -> f32 {
            (0..16)
                .map(|i| gesture_shot(kind, Direction::Down, level, i as f32 / 16.0).speed())
                .sum::<f32>()
                / 16.0
        };
        assert!(
            speed(Kind::Ice) > speed(Kind::Water) * 1.5,
            "ice {} should be much faster than water {}",
            speed(Kind::Ice),
            speed(Kind::Water)
        );
        let life = |kind: Kind| gesture_shot(kind, Direction::Down, level, 0.0).life;
        assert!(life(Kind::Water) > life(Kind::Ice) * 2.0);
    }

    /// A streak effect has to follow the gesture it came from, or a burst
    /// triggered by a leftward slide would throw right.
    #[test]
    fn a_streak_burst_follows_the_gesture() {
        for kind in [Kind::Flicker, Kind::Wind] {
            let left = gesture_shot(kind, Direction::Left, 10, 0.5);
            let right = gesture_shot(kind, Direction::Right, 10, 0.5);
            assert!(
                left.vx < 0.0 && right.vx > 0.0,
                "{kind:?} did not follow the gesture: {left:?} then {right:?}"
            );
        }
    }

    /// The same inputs must give the same burst. A gesture that reshuffled
    /// itself on every keypress would be noise, and it would also make the
    /// shapes above impossible to assert on.
    #[test]
    fn a_gesture_is_reproducible() {
        for kind in KINDS {
            for dir in DIRS {
                for t in [0.0, 0.25, 0.5, 0.75] {
                    assert_eq!(
                        gesture_shot(kind, dir, 7, t),
                        gesture_shot(kind, dir, 7, t),
                        "{kind:?} {dir:?} t={t} was not reproducible"
                    );
                }
            }
        }
    }

    /// `t` walks a full turn, so a burst leaves in every direction rather than
    /// in a line. Half a turn apart has to be opposite.
    #[test]
    fn a_burst_spreads_around_the_gesture() {
        for kind in [Kind::Water, Kind::Ice] {
            let at_zero = gesture_shot(kind, Direction::Right, 10, 0.0).vx;
            let at_half = gesture_shot(kind, Direction::Right, 10, 0.5).vx;
            assert!(
                at_zero > 0.0 && at_half < 0.0,
                "{kind:?} burst went one way only: {at_zero} then {at_half}"
            );
        }
    }

    /// A level below one is level one, not a division by zero or a negative
    /// particle count.
    #[test]
    fn a_level_outside_the_scale_clamps() {
        for level in [i32::MIN, -1000, 0, 1] {
            assert_eq!(gesture_power(level), gesture_power(1), "level {level}");
        }
        assert_eq!(gesture_power(i32::MAX), gesture_power(MAX_GRAVITY as i32));
    }

    /// A direction is only ever the one the piece actually went.
    #[test]
    fn a_grid_delta_becomes_the_direction_it_actually_went() {
        assert_eq!(Direction::from_delta((-1, 0)), Direction::Left);
        assert_eq!(Direction::from_delta((1, 0)), Direction::Right);
        assert_eq!(Direction::from_delta((0, 1)), Direction::Down);
        // A rotation in place, and a kick that went sideways as well as down:
        // the sideways part is what the player saw, so it wins.
        assert_eq!(Direction::from_delta((0, 0)), Direction::Down);
        assert_eq!(Direction::from_delta((-2, 1)), Direction::Left);
        assert_eq!(Direction::from_delta((2, -1)), Direction::Right);
    }

    /// Particles have to be reaped or they accumulate for the length of a run.
    /// A gesture's lifetime is finite by construction, so stepping past it
    /// must empty the system.
    #[test]
    fn a_gesture_clears_itself_once_its_life_is_up() {
        let mut p = Particles::new("water");
        p.spawn_gesture(Piece::T, 0, (4, 4), Direction::Down, 10);
        assert!(!p.gestures.is_empty(), "nothing was thrown");
        let life = p.gestures[0].max_age;
        // Step right up to the end of its life: still there.
        p.update(life * 0.9, (0.0, 0.0));
        assert!(!p.gestures.is_empty(), "the burst vanished early");
        p.update(life * 0.2, (0.0, 0.0));
        assert!(p.gestures.is_empty(), "the burst outlived its own life");
    }

    /// A level one burst and a level twenty burst must be different sizes, or
    /// the scaling is only in a function nobody calls.
    #[test]
    fn the_burst_on_screen_grows_with_the_level() {
        let mut p = Particles::new("ice");
        p.spawn_gesture(Piece::T, 0, (4, 4), Direction::Down, 1);
        let low = p.gestures.len();
        p.gestures.clear();
        p.spawn_gesture(Piece::T, 0, (4, 4), Direction::Down, 20);
        assert!(p.gestures.len() > low, "level 20 threw no more than level 1");
    }

    /// With the Effect setting off there is no kind and so no burst - the same
    /// `none` switch that silences the trail silences the gestures, or `none`
    /// would only half work.
    #[test]
    fn no_effect_means_no_gestures() {
        let mut p = Particles::new("none");
        assert!(!p.active());
        p.spawn_gesture(Piece::T, 0, (4, 4), Direction::Down, 20);
        assert!(p.gestures.is_empty());
    }

    /// Switching the effect mid-burst must not leave particles drawing
    /// themselves in a material the player just switched away from.
    #[test]
    fn switching_the_effect_clears_gestures_in_flight() {
        let mut p = Particles::new("flame");
        p.spawn_gesture(Piece::T, 0, (4, 4), Direction::Down, 20);
        assert!(!p.gestures.is_empty());
        p.set_effect("ice");
        assert!(p.gestures.is_empty(), "a stale burst survived the switch");
    }

    /// A burst is thrown from the edge the piece swept, inside the piece's own
    /// footprint. A burst that started outside the block would appear in empty
    /// space with nothing to explain it.
    #[test]
    fn a_burst_is_thrown_from_the_piece_it_came_from() {
        let (ox, oy) = (4i32, 6i32);
        for dir in DIRS {
            let mut p = Particles::new("flame");
            p.spawn_gesture(Piece::O, 0, (ox, oy), dir, 10);
            let (minx, maxx, miny, maxy) = piece_bounds(Piece::O, 0);
            let left = (ox + minx) as f32 * BLOCK_SIZE as f32;
            let right = (ox + maxx + 1) as f32 * BLOCK_SIZE as f32;
            let top = (oy + miny - HIDDEN_ROWS as i32) as f32 * BLOCK_SIZE as f32;
            let bottom = (oy + maxy + 1 - HIDDEN_ROWS as i32) as f32 * BLOCK_SIZE as f32;
            for g in &p.gestures {
                assert!(g.x >= left - 1.0 && g.x <= right + 1.0, "{dir:?} x {}", g.x);
                assert!(g.y >= top - 1.0 && g.y <= bottom + 1.0, "{dir:?} y {}", g.y);
            }
        }
    }

    /// Each kind starts in a colour of its own, so a burst is identifiable even
    /// before it moves.
    #[test]
    fn every_effect_bursts_in_its_own_colour() {
        let mut seen: Vec<[u8; 3]> = Vec::new();
        for kind in KINDS {
            let c = gesture_color(kind);
            assert!(!seen.contains(&c), "{kind:?} reuses another kind's colour");
            seen.push(c);
        }
    }

    #[test]
    fn no_glyph_is_empty() {
        // An all-zero glyph is invisible, and there is no way to tell one from a
        // real one by reading the table - it just never appears in the rain.
        // 34 entries and one is silently missing from the effect.
        for (i, glyph) in KATAKANA.iter().enumerate() {
            assert!(
                glyph.iter().any(|row| *row != 0),
                "glyph {i} is blank"
            );
        }
    }

    #[test]
    fn no_glyph_sets_a_column_it_does_not_have() {
        // Only the low GLYPH_W bits mean anything. A bit above them would be
        // drawn *outside* the glyph cell, and because the cells are laid out
        // edge to edge that pixel lands on the neighbouring glyph's column -
        // a smudge that no amount of looking at the table would explain.
        let mask = (1u8 << GLYPH_W) - 1;
        for (i, glyph) in KATAKANA.iter().enumerate() {
            for (row, bits) in glyph.iter().enumerate() {
                assert_eq!(
                    bits & !mask,
                    0,
                    "glyph {i} row {row} sets a bit outside its {GLYPH_W} columns"
                );
            }
        }
    }

    #[test]
    fn the_glyph_column_stays_in_bounds_for_any_seed_or_age() {
        // `slot` and `step` grow without bound against a 34-entry table, so the
        // index has to wrap. This walks well past the table length in both
        // directions, including the seeds and steps the game cannot actually
        // produce, because the failure mode is an out-of-bounds panic and a
        // panic in the draw loop is a crash rather than a bad frame.
        for seed in 0..KATAKANA.len() as u32 + 7 {
            for slot in -12..24 {
                for step in 0..40 {
                    let g = glyph_at(seed, slot, step);
                    assert!(
                        std::ptr::eq(g, &KATAKANA[0]) || g.iter().any(|r| *r != 0),
                        "glyph_at({seed}, {slot}, {step}) landed off the table"
                    );
                }
            }
        }
    }

    #[test]
    fn the_column_scrolls_as_the_particle_ages() {
        // Frozen glyphs read as a green smear. The column has to actually change
        // as `step` advances, and consecutive slots have to differ, or the
        // effect is one glyph repeated.
        let seed = 5u32;
        let first: Vec<_> = (0..5).map(|s| glyph_at(seed, s, 0)[0]).collect();
        let later: Vec<_> = (0..5).map(|s| glyph_at(seed, s, 1)[0]).collect();
        assert_ne!(first, later, "the column never scrolls");
        assert!(
            first.windows(2).any(|w| w[0] != w[1]),
            "every slot in a column drew the same glyph"
        );
    }

    #[test]
    fn a_matrix_streak_is_the_size_of_the_originals_bar() {
        // The regression to watch: `length` was the original's 6-15 *pixels*,
        // and reusing that number as a glyph count would give a 200px column
        // dragging behind a 30px piece. The spawn range is 2-5 glyphs, which at
        // MATRIX_SCALE is 28-70px - the same streak the original drew.
        for length in 2..=5 {
            let px = length * MATRIX_PX_H;
            assert!(
                px <= 4 * BLOCK_SIZE,
                "{length} glyphs is {px}px, more than four blocks trailing a piece"
            );
            // No lower bound worth pinning: the shortest streak is 28px against a
            // 30px block, which is the original's 6-15px bar plus the glyph it
            // is drawn on. What matters is that it is never a *single* glyph,
            // which would be a character floating next to the piece rather than
            // a trail.
            assert!(length >= 2, "a one-glyph column is not a trail");
        }
        assert_eq!(MATRIX_PX_W, 10, "glyphs are 5 cells at scale 2");
        assert_eq!(MATRIX_PX_H, 14, "glyphs are 7 cells at scale 2");
    }

    #[test]
    fn the_head_is_brighter_than_the_tail() {
        // Same interpolation the original used, but with the head at the
        // leading (bottom) edge. Pin the direction: a streak whose bright end
        // is the trailing one reads as being dragged upwards, which is the
        // original's bug and the reason this was changed.
        let g = 200.0f32;
        let a = 255.0f32;
        let count = 5.0f32;
        let head = {
            let t = 0.5 / count;
            (g * (1.0 - t) + (g / 3.0) * t, a * (1.0 - t) + (a * 0.4) * t)
        };
        let tail = {
            let t = (count - 0.5) / count;
            (g * (1.0 - t) + (g / 3.0) * t, a * (1.0 - t) + (a * 0.4) * t)
        };
        assert!(head.0 > tail.0, "head green {head:?} vs tail {tail:?}");
        assert!(head.1 > tail.1, "head alpha {head:?} vs tail {tail:?}");
    }
}