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
use crate::config::{CONTENT_WIDTH, SCREEN_HEIGHT, SCREEN_WIDTH, SUBWINDOW_WIDTH};

/// How many numbered background slots to look at (`1.jpg` .. `=COUNT`).
///
/// Only a cap: numbering normally stops at the first slot that has neither a
/// file on disk nor a compiled-in copy, which is how a player adding `16.jpg`
/// through `20.jpg` gets five more.
const MAX_BACKGROUNDS: u32 = 64;

/// Black overlay over the photo behind the playfield, 0-255. Not cosmetic:
/// blocks, the ghost piece and the grid lines all have to stay readable against
/// an arbitrary picture, so there has to be enough of it to flatten any photo
/// into a backdrop.
///
/// 100 - about 39% - is unchanged. It was 140 (55%) before, which crushed the
/// images to near-black without buying any more readability, since the blocks
/// and the lattice are both drawn at full alpha over the top either way.
///
/// Raising it was never the answer to "the background never changes with the
/// level", though, and lowering it globally is not either: dimming harder is
/// what made the change invisible in the first place. See [`DIM_PANEL`].
const DIM_WELL: u8 = 100;

/// Black overlay over the photo behind the side panel, 0-255.
///
/// The panel has no blocks in it, so none of the legibility argument for
/// [`DIM_WELL`] applies to it, and dimming it as hard as the well threw away the
/// one part of the screen where a photo is unobstructed.
///
/// This is the actual fix for the photo being invisible. Measured over all
/// fifteen bundled photos, the average luminance after [`DIM_WELL`] sits between
/// 36 and 53 out of 255 - the dimmed photo is between 14% and 21% grey, which is
/// what "a new photo appeared" looks like when it is nearly black to begin
/// with. The panel is the only region large enough to show a whole frame of a
/// picture, and it holds the high score, the score and the level, so it is also
/// the region a player is looking at when a level changes. At 30 the same
/// fifteen photos land between 52 and 76, which is plainly a photograph.
const DIM_PANEL: u8 = 30;

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
    /// overflow) so no letterboxing shows, then dimmed - in two zones, not one.
    /// See [`DIM_WELL`] and [`DIM_PANEL`]: the well needs the overlay to keep
    /// the blocks readable, and the panel does not, so darkening it as well only
    /// made the photo too dim to notice changing.
    ///
    /// It covers [`CONTENT_WIDTH`] x [`SCREEN_HEIGHT`], the whole render texture,
    /// not just the well. Fitting it to the well alone left the entire side panel
    /// on the plain backdrop, so the only visible part of the photo was the strip
    /// behind the stack, and a photo that changed with the level was all but
    /// invisible.
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
        let (x, y, dw, dh) = cover_rect(tw, th, CONTENT_WIDTH as f32, SCREEN_HEIGHT as f32);

        d.draw_texture_pro(
            tex,
            Rectangle::new(0.0, 0.0, tw, th),
            Rectangle::new(x, y, dw, dh),
            Vector2::new(0.0, 0.0),
            0.0,
            Color::WHITE,
        );
        // Two rectangles rather than one over the whole texture, so the split
        // falls exactly on the well's right-hand edge. The panel's rectangle
        // starts there and runs to the end, and the two together cover the same
        // area the single rectangle used to, so no seam and no uncovered strip.
        d.draw_rectangle_rec(
            Rectangle::new(0.0, 0.0, SCREEN_WIDTH as f32, SCREEN_HEIGHT as f32),
            Color::new(0, 0, 0, DIM_WELL),
        );
        d.draw_rectangle_rec(
            Rectangle::new(
                SCREEN_WIDTH as f32,
                0.0,
                SUBWINDOW_WIDTH as f32,
                SCREEN_HEIGHT as f32,
            ),
            Color::new(0, 0, 0, DIM_PANEL),
        );
    }
}

/// Cover-fit a `tw` x `th` image into a `fw` x `fh` area.
///
/// Returns the destination rect as `(x, y, w, h)`: scaled up by whichever axis
/// needs it more, then centred, so the area is always completely covered and the
/// overflow is cropped off evenly rather than letterboxed.
///
/// Split out of [`Backgrounds::draw`] because the arithmetic is the part worth
/// testing, and testing it needs no graphics context.
fn cover_rect(tw: f32, th: f32, fw: f32, fh: f32) -> (f32, f32, f32, f32) {
    let scale = (fw / tw).max(fh / th);
    let (dw, dh) = (tw * scale, th * scale);
    ((fw - dw) / 2.0, (fh - dh) / 2.0, dw, dh)
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

    /// The photo has to reach the edges of the render texture on both axes.
    ///
    /// This is the regression for "the background never changes to a new level".
    /// Cover-fitting to the playfield width alone was correct arithmetic and looked
    /// fine in isolation, but it stopped at the right-hand edge of the well and
    /// left the whole side panel on the plain backdrop, so the visible part of the
    /// photo was a narrow strip behind the stack and a new photo for a new level
    /// was effectively invisible.
    #[test]
    fn the_photo_covers_the_whole_render_target() {
        let fw = CONTENT_WIDTH as f32;
        let fh = SCREEN_HEIGHT as f32;
        assert!(
            CONTENT_WIDTH > crate::config::SCREEN_WIDTH,
            "the content width has to include the side panel, or there is nothing \
             to draw a background behind"
        );

        // A few shapes, including the square and both landscape orientations, so
        // the fit cannot be tuned for just one of them.
        for (tw, th) in [(1024.0, 1024.0), (1920.0, 1080.0), (1080.0, 1920.0), (800.0, 600.0)] {
            let (x, y, dw, dh) = cover_rect(tw, th, fw, fh);
            assert!(
                dw >= fw - 0.5 && dh >= fh - 0.5,
                "{tw}x{th} left a gap: drew {dw}x{dh} into {fw}x{fh}"
            );
            assert!(
                x <= 0.5 && y <= 0.5,
                "{tw}x{th} did not reach the top-left corner: at ({x}, {y})"
            );
            // Centred, so the crop is split evenly between the two sides.
            assert!(
                (x - ((fw - dw) / 2.0)).abs() < 0.01 && (y - ((fh - dh) / 2.0)).abs() < 0.01,
                "{tw}x{th} was not centred"
            );
            // The aspect ratio has to survive, or the photo is stretched.
            let want = tw / th;
            let got = dw / dh;
            assert!(
                (want - got).abs() / want < 0.01,
                "{tw}x{th} came out as {dw}x{dh}, aspect {got} not {want}"
            );
        }
    }

    /// Every photo compiled into the executable has to decode, and there has to
    /// be more than one of them.
    ///
    /// This is the guard on "the background never changes to a new level".
    /// [`Backgrounds::load`] stops at the first slot that will not decode, so a
    /// single unreadable photo would leave the game with one image and
    /// [`choose_index`] nothing to choose between: the same picture at every
    /// level, forever, with nothing on screen to explain it. The release build is
    /// a GUI-subsystem binary, so the warning that would have said so is written
    /// to a stderr nobody is reading.
    ///
    /// Decoding is pure stb_image work on the CPU and needs no graphics context,
    /// so this checks the whole set without opening a window.
    #[test]
    fn every_embedded_photo_decodes_and_there_is_more_than_one() {
        let count = assets::background_count();
        assert!(
            count > 1,
            "only {count} photo(s) compiled in, so the background cannot vary"
        );
        for n in 1..=count {
            let bytes = assets::background(n)
                .unwrap_or_else(|| panic!("{n}.jpg is named by the asset table but not embedded"));
            let image = Image::load_image_from_mem(assets::IMAGE_FORMAT, bytes)
                .unwrap_or_else(|e| panic!("{n}.jpg did not decode: {e}"));
            assert!(
                image.width() > 0 && image.height() > 0,
                "{n}.jpg decoded to an empty image"
            );
        }
    }

    /// Average Rec. 601 luminance, 0-255, of embedded photo `n`.
    ///
    /// Sampled on a stride rather than read whole: these are 1408x768 photos,
    /// all fifteen of them, and a full pass over every pixel on each test run
    /// buys nothing over a thousand evenly spaced samples of a photographic
    /// image. The stride is coprime with both dimensions, so the samples walk
    /// diagonals instead of landing on the same column of every row.
    #[cfg(test)]
    fn photo_luma(n: u32) -> f64 {
        let bytes = assets::background(n).unwrap_or_else(|| panic!("{n}.jpg is not embedded"));
        let image = Image::load_image_from_mem(assets::IMAGE_FORMAT, bytes)
            .unwrap_or_else(|e| panic!("{n}.jpg did not decode: {e}"));
        let mut sum = 0.0f64;
        let mut count = 0u64;
        for (i, c) in image.get_image_data().iter().enumerate() {
            if i % 997 == 0 {
                sum += 0.299 * c.r as f64 + 0.587 * c.g as f64 + 0.114 * c.b as f64;
                count += 1;
            }
        }
        assert!(count > 100, "{n}.jpg gave too few samples to measure");
        sum / count as f64
    }

    /// The same luminance after the `dim` black overlay is laid over it.
    #[cfg(test)]
    fn dimmed(avg: f64, dim: u8) -> f64 {
        avg * (255.0 - dim as f64) / 255.0
    }

    /// A dimmed photo has to still be a photograph.
    ///
    /// This is the guard on "I didn't notice the backgrounds changing with each
    /// new level", and it is written against measurement rather than taste. At a
    /// flat 100 overlay every bundled photo sat between 14% and 21% grey once
    /// dimmed - a photo that is nearly black to start with, dimmed into being
    /// invisible, changing to a different photo that is also nearly black. The
    /// player was being shown a new picture every ten lines and could not tell.
    ///
    /// The panel is where the photo is allowed to be bright, because it is the
    /// only region with nothing drawn over it and the only one wide enough to
    /// show a whole frame.
    #[test]
    fn the_panel_is_bright_enough_for_the_photo_to_read_as_a_photo() {
        let mut dimmest = f64::MAX;
        let mut brightest = f64::MIN;
        for n in 1..=assets::background_count() {
            let luma = dimmed(photo_luma(n), DIM_PANEL);
            assert!(
                luma >= 45.0,
                "{n}.jpg is only {luma:.0}/255 behind the panel, which is near black \
                 rather than a photograph"
            );
            dimmest = dimmest.min(luma);
            brightest = brightest.max(luma);
        }
        // A player has to be able to tell two photos apart, not just see one.
        // This is the same reason `choose_index` refuses to repeat the level
        // below: swapping is only worth anything if the swap is visible.
        assert!(
            brightest - dimmest >= 20.0,
            "every photo lands between {dimmest:.0} and {brightest:.0} behind the panel, \
             a spread of {:.0}/255, so a new photo looks like the old one",
            brightest - dimmest
        );
    }

    /// The well must not get brighter to fix the panel.
    ///
    /// The dim over the playfield is what keeps blocks, the ghost piece and the
    /// grid lines readable against an arbitrary photograph, and it is the one
    /// part of the screen where that argument applies. Both bounds are
    /// load-bearing: too dark and the well is the near-black region the panel
    /// test is complaining about, too light and a bright photo starts pulling
    /// attention off the stack, which is what the original's 140 did.
    #[test]
    fn the_well_stays_a_dark_backdrop_for_the_blocks() {
        for n in 1..=assets::background_count() {
            let luma = dimmed(photo_luma(n), DIM_WELL);
            assert!(
                (30.0..=60.0).contains(&luma),
                "{n}.jpg is {luma:.0}/255 behind the well, outside the 30-60 band a \
                 readable backdrop needs"
            );
        }
    }

    /// The two overlays have to meet exactly, with no seam and no gap.
    ///
    /// A dim that stopped short of the texture's right edge would leave a strip
    /// of undimmed photo, and one that overlapped would darken the panel's
    /// first column back to the well's value. The arithmetic is the whole of it,
    /// which is why it is a test and not a comment.
    #[test]
    fn the_two_dims_tile_the_render_target_exactly() {
        let well = SCREEN_WIDTH as f32;
        let panel = SUBWINDOW_WIDTH as f32;
        assert_eq!(
            well + panel,
            CONTENT_WIDTH as f32,
            "the two dim zones have to add up to the whole render target"
        );
        // And the well's dim must not be lighter than the panel's, or the panel
        // would be a visibly darker band rather than the bright one.
        assert!(
            DIM_PANEL < DIM_WELL,
            "the panel overlay ({DIM_PANEL}) is not lighter than the well's \
             ({DIM_WELL}), so the photo is dimmed hardest where it is most visible"
        );
    }
}
