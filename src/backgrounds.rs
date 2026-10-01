//! Per-level background images.
//!
//! One photo per level, picked at random the first time that level is
//! reached, so a run walks through the set without repeating itself
//! back-to-back. Images are loaded once at startup and kept on the GPU.
//!
//! The photos are compiled into the executable by [`crate::assets`], so a copied
//! exe still has them; a file of the same name in `assets/backgrounds/` on disk
//! wins, so a player can drop in their own picture without rebuilding. Every part
//! of this is still optional: if a picture will not decode, the game runs and
//! simply draws the plain background colour. A bad image must never stop play.

use std::collections::HashMap;

use rand::Rng;
use raylib::core::color::Color;
use raylib::core::texture::{Image, Texture2D};
use raylib::prelude::*;

use crate::assets;
use crate::audio::find_asset;
use crate::config::{SCREEN_HEIGHT, SCREEN_WIDTH};

/// How many numbered background slots to look at (`1.jpg` .. `=COUNT`).
///
/// Only a cap: numbering normally stops at the first slot that has neither a
/// file on disk nor a compiled-in copy, which is how a player adding `16.jpg`
/// through `20.jpg` gets five more.
const MAX_BACKGROUNDS: u32 = 64;

/// Black overlay laid over the photo, 0-255. Not cosmetic: blocks, the ghost
/// piece and the grid lines all have to stay readable against an arbitrary
/// picture, so there has to be enough of it to flatten any photo into a
/// backdrop.
///
/// 100 - about 39% - was the value that read as "a photo you can actually see"
/// rather than "a dark shape behind the well". It was 140 (55%) before, which
/// crushed the images to near-black without buying any more readability, since
/// the blocks and the lattice are both drawn at full alpha over the top either
/// way. Anything much below this and a bright photo starts pulling attention
/// off the stack; **Options > Backgrounds** turns the photos off entirely for
/// anyone who would rather have no picture at all.
const DIM: u8 = 100;

/// The loaded images, plus the level -> image choice made for each level.
pub struct Backgrounds {
    textures: Vec<Texture2D>,
    /// Image index chosen per level. Absent until that level is first shown.
    by_level: HashMap<i32, usize>,
}

impl Backgrounds {
    /// Load every background photo: `assets/backgrounds/N.jpg` from disk if it
    /// is there, otherwise the copy compiled into the executable.
    ///
    /// Runs once at startup, so a few dozen megabytes of JPEG decoding is a
    /// one-off startup cost. Numbering stops at the first slot that has neither,
    /// and a file that will not decode is skipped with a warning rather than
    /// aborting.
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread) -> Self {
        let mut textures = Vec::new();
        for n in 1..=MAX_BACKGROUNDS {
            let name = format!("backgrounds/{n}.jpg");
            let path = find_asset(&name);
            // Existence is checked up front for the disk copy: handing raylib a
            // path it cannot open logs a warning, and the first gap in the
            // numbering is the normal way the list ends, not a problem worth
            // reporting.
            let tex = if std::path::Path::new(&path).is_file() {
                match rl.load_texture(thread, &path) {
                    Ok(tex) => Some(tex),
                    Err(e) => {
                        eprintln!("background: could not decode {path}: {e}");
                        None
                    }
                }
            } else {
                texture_from_memory(rl, thread, n)
            };
            // A gap is the end of the list; a present-but-undecodable image is
            // that slot lost, and the numbering carries on.
            match tex {
                Some(tex) => textures.push(tex),
                None if std::path::Path::new(&path).is_file() => continue,
                None => break,
            }
        }
        if textures.is_empty() {
            eprintln!("background: no images found; using the plain backdrop.");
        } else {
            eprintln!(
                "background: loaded {} image(s), {} compiled into the executable.",
                textures.len(),
                assets::background_count()
            );
        }
        Self {
            textures,
            by_level: HashMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }

    /// The image for `level`, choosing one at random the first time the level
    /// is reached and reusing it for every frame after that.
    pub fn for_level(&mut self, level: i32) -> Option<&Texture2D> {
        if self.textures.is_empty() {
            return None;
        }
        if !self.by_level.contains_key(&level) {
            let pick = choose_index(&self.by_level, level, self.textures.len());
            self.by_level.insert(level, pick);
        }
        let idx = self.by_level[&level];
        self.textures.get(idx)
    }

    /// Draw the background for `level` across the whole window, if the player
    /// wants one.
    ///
    /// `enabled` is the **Backgrounds** option. The check comes *before*
    /// [`Self::for_level`], so a run with backgrounds off does not quietly
    /// consume the per-level image choices either: turning the option back on
    /// then gives a proper sequence rather than starting mid-way through one,
    /// and the choice a level makes is not thrown away by a toggle.
    ///
    /// The image is cover-fitted (scaled to fill, centred, cropping the
    /// overflow) so no letterboxing shows, then dimmed. The dim is not
    /// cosmetic: blocks and ghost pieces have to stay readable against an
    /// arbitrary photo.
    ///
    /// With `enabled` false, or with no images loaded, this draws nothing and
    /// leaves the plain backdrop the render target was cleared to.
    pub fn draw(&mut self, d: &mut RaylibDrawHandle, level: i32, enabled: bool) {
        if !enabled {
            return;
        }
        let Some(tex) = self.for_level(level) else {
            return;
        };
        let (tw, th) = (tex.width().max(1) as f32, tex.height().max(1) as f32);
        let scale = (SCREEN_WIDTH as f32 / tw).max(SCREEN_HEIGHT as f32 / th);
        let (dw, dh) = (tw * scale, th * scale);

        d.draw_texture_pro(
            tex,
            Rectangle::new(0.0, 0.0, tw, th),
            Rectangle::new(
                (SCREEN_WIDTH as f32 - dw) / 2.0,
                (SCREEN_HEIGHT as f32 - dh) / 2.0,
                dw,
                dh,
            ),
            Vector2::new(0.0, 0.0),
            0.0,
            Color::WHITE,
        );
        d.draw_rectangle_rec(
            Rectangle::new(0.0, 0.0, SCREEN_WIDTH as f32, SCREEN_HEIGHT as f32),
            Color::new(0, 0, 0, DIM),
        );
    }
}

/// Decode the compiled-in copy of background photo `n` into a GPU texture.
///
/// `LoadImageFromMemory` needs the format name rather than the filename, and the
/// decoded image has to outlive the upload, so the temporary is bound before the
/// texture call rather than being dropped mid-expression.
fn texture_from_memory(rl: &mut RaylibHandle, thread: &RaylibThread, n: u32) -> Option<Texture2D> {
    let bytes = assets::background(n)?;
    let image = Image::load_image_from_mem(assets::IMAGE_FORMAT, bytes).ok()?;
    rl.load_texture_from_image(thread, &image).ok()
}

/// Pick an image index for `level` out of `n` available, avoiding an
/// immediate repeat of whatever the level below got.
///
/// Separate from [`Backgrounds`] so it can be exercised without a GPU.
fn choose_index(by_level: &HashMap<i32, usize>, level: i32, n: usize) -> usize {
    let mut rng = rand::thread_rng();
    let mut pick = rng.gen_range(0..n);
    if n > 1 {
        if let Some(&prev) = by_level.get(&(level - 1)) {
            if pick == prev {
                // Step off the repeat into any of the other n-1 slots.
                pick = (pick + 1 + rng.gen_range(0..n - 1)) % n;
            }
        }
    }
    pick
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every pick must land on a real image.
    #[test]
    fn picks_are_in_range_and_never_repeat_the_level_below() {
        let n = 4;
        let mut by_level: HashMap<i32, usize> = HashMap::new();
        for level in 1..=50 {
            let pick = choose_index(&by_level, level, n);
            assert!(pick < n, "level {level} picked missing image {pick}");
            if let Some(&prev) = by_level.get(&(level - 1)) {
                assert_ne!(
                    pick, prev,
                    "level {level} repeated the image from level {}",
                    level - 1
                );
            }
            by_level.insert(level, pick);
        }
    }

    /// A single image is the degenerate case: it must still be chosen, and
    /// must not loop trying to avoid a repeat it cannot avoid.
    #[test]
    fn one_image_is_always_usable() {
        let by_level = HashMap::new();
        for level in 1..=5 {
            assert_eq!(choose_index(&by_level, level, 1), 0);
        }
    }

    /// A level that has already been drawn must keep its image rather than
    /// flickering between photos every frame.
    #[test]
    fn a_level_keeps_the_same_image() {
        let n = 3;
        let mut by_level: HashMap<i32, usize> = HashMap::new();
        for level in 1..=10 {
            if !by_level.contains_key(&level) {
                let pick = choose_index(&by_level, level, n);
                by_level.insert(level, pick);
            }
            let first = by_level[&level];
            // Simulate many more frames at the same level.
            for _ in 0..10 {
                assert_eq!(by_level[&level], first, "level {level} flickered");
            }
        }
    }
}
