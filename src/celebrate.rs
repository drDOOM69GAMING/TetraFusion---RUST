//! The new-high-score celebration: tetromino shards thrown across the layout.
//!
//! This one is new, and it is not a particle effect in [`crate::effects`] because
//! of *where* it is. Every system in there works in playfield cell space, with a
//! screen-shake offset, because it is reporting something that happened to a
//! piece. A record is not a thing that happened to a piece - it happened to the
//! run - so this works in layout pixels across the whole [`CONTENT_WIDTH`] of the
//! screen, well and side panel both, and is drawn behind the initials prompt
//! rather than in front of it.
//!
//! The shards are real tetrominoes, taken from [`crate::pieces`] rather than
//! redrawn, for the same reason the well is drawn with the game's own blocks: a
//! celebration made of the thing being celebrated reads as part of the game,
//! where a shower of coloured squares reads as a screensaver.
//!
//! Everything here is inert until [`Celebration::start`] is called, which only
//! happens when a run beats the standing record for its mode. A run that does
//! not set a record throws nothing at all.

use rand::Rng;
use raylib::prelude::*;

use crate::config::{COLORS, CONTENT_WIDTH, HIGH_SCORE_CELEBRATION_MS, SCREEN_HEIGHT};
use crate::pieces::{self, Piece, ALL_PIECES};

/// Edge of one block within a shard, in pixels.
///
/// Small against the [`crate::config::BLOCK_SIZE`] of a real piece on purpose:
/// about a third of it, so a shard is recognisably a tetromino at a glance but
/// reads as debris rather than as a piece the player might mistake for a falling
/// one. The shards also spawn and die well away from the floor, so there is no
/// moment where one could be read as playable.
const SHARD_CELL: f32 = 10.0;

/// Shards thrown per second while the celebration runs.
///
/// Eighteen, which is three every frame at 60 Hz. Enough that the screen is
/// never empty, few enough that the stack behind them is still visible - this
/// plays over a board the player is about to look at for the initials and the
/// standing high score, and burying that under confetti would be a worse bug
/// than no celebration at all.
const RATE: f32 = 18.0;

/// How long a shard is allowed to be airborne, in seconds, before its next
/// throw. The shortest life is what sets how quickly the screen clears once the
/// window closes.
const LIFE_MIN: f32 = 0.9;
const LIFE_MAX: f32 = 1.7;

/// Launch speed range, in pixels a second.
const SPEED_MIN: f32 = 180.0;
const SPEED_MAX: f32 = 520.0;

/// Downward acceleration, in pixels a second squared.
///
/// Not the game's gravity and not trying to be: this is debris in air, and at
/// [`SPEED_MIN`] a shard thrown straight up is still within a tenth of a second
/// of coming back down, which is what makes a burst read as an explosion rather
/// than as a fountain.
const FALL: f32 = 420.0;

/// Spin range, in degrees a second. Sign is picked per shard, so roughly half
/// tumble each way.
const SPIN_MIN: f32 = 90.0;
const SPIN_MAX: f32 = 420.0;

/// Fraction of a shard's life spent fading in, and the fraction spent fading
/// out. What is left over - over half, with these - is spent at full strength.
/// See [`Shard::alpha`].
const FADE_IN: f32 = 0.08;
const FADE_OUT: f32 = 0.4;

/// One tetromino in flight.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Shard {
    /// Centre of the shard's own bounding box, in layout pixels.
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    /// Current angle in degrees, and how fast it is changing.
    rot: f32,
    spin: f32,
    piece: Piece,
    /// Which of the piece's four rotation states it is thrown in. Part of the
    /// shard rather than drawn from at random per frame, so the shape is a
    /// tetromino and stays one instead of scrambling.
    shape: usize,
    color: [u8; 3],
    age: f32,
    life: f32,
}

impl Shard {
    /// How far through its own life it is, `0.0..=1.0`.
    fn progress(&self) -> f32 {
        (self.age / self.life).clamp(0.0, 1.0)
    }

    /// Alpha to draw it at, 0-255.
    ///
    /// Fades in over the first [`FADE_IN`] of its life, sits at full strength,
    /// then fades out over the last [`FADE_OUT`]. Both ends matter and neither
    /// alone is enough:
    ///
    /// The fade-in is what stops a burst popping. Several shards are thrown
    /// every frame, so without it each one appears at full strength out of
    /// nothing and the burst reads as a flicker rather than a bloom.
    ///
    /// The long flat middle is what stops it looking washed out. A curve that
    /// fades in and straight back out never gets past half strength - the peak of
    /// `t * (1 - t)` is 0.25, so every shard on screen is at 25% alpha at best,
    /// and over a dark backdrop that is a faint smear rather than a piece.
    fn alpha(&self) -> u8 {
        let t = self.progress();
        let rise = (t / FADE_IN).min(1.0);
        let fall = ((1.0 - t) / FADE_OUT).clamp(0.0, 1.0);
        (255.0 * rise * fall).clamp(0.0, 255.0) as u8
    }

    /// The four cells, centred on the shard and rotated by `rot`.
    ///
    /// Returns `(x, y)` in layout pixels for each of the piece's four blocks.
    /// Split out as a pure function of the shard so the geometry can be checked
    /// without a window: the offsets, the rotation and the centring are the
    /// whole of it, and they are all things a wrong answer would still *look*
    /// like something.
    fn cells(&self) -> [(f32, f32); 4] {
        let raw = pieces::cells(self.piece, self.shape);
        // Centre the piece on the shard's own position. The bounding box of
        // `cells` is what gets centred, not the average of the four cells: the
        // I piece is 4x1 and the O is 2x2, and averaging would make them sit
        // off to one side of their own centre of mass.
        let (mut minx, mut maxx) = (i32::MAX, i32::MIN);
        let (mut miny, mut maxy) = (i32::MAX, i32::MIN);
        for (cx, cy) in raw {
            minx = minx.min(cx);
            maxx = maxx.max(cx);
            miny = miny.min(cy);
            maxy = maxy.max(cy);
        }
        let midx = (minx + maxx) as f32 / 2.0;
        let midy = (miny + maxy) as f32 / 2.0;

        let rad = self.rot.to_radians();
        let (sin, cos) = rad.sin_cos();
        let mut out = [(0.0f32, 0.0f32); 4];
        for (i, (cx, cy)) in raw.iter().enumerate() {
            // Cell centres, in units of one block, relative to the box centre.
            let (lx, ly) = (*cx as f32 - midx, *cy as f32 - midy);
            out[i] = (
                self.x + (lx * cos - ly * sin) * SHARD_CELL,
                self.y + (lx * sin + ly * cos) * SHARD_CELL,
            );
        }
        out
    }

    fn draw(&self, d: &mut RaylibDrawHandle) {
        let a = self.alpha();
        if a == 0 {
            return;
        }
        let colour = Color::new(self.color[0], self.color[1], self.color[2], a);
        // Rotated about the shard's own centre, so the whole piece turns as one
        // thing rather than each block spinning in place.
        let origin = Vector2::new(self.x, self.y);
        for (cx, cy) in self.cells() {
            d.draw_rectangle_pro(
                Rectangle::new(
                    cx - SHARD_CELL / 2.0,
                    cy - SHARD_CELL / 2.0,
                    SHARD_CELL,
                    SHARD_CELL,
                ),
                origin,
                self.rot,
                colour,
            );
        }
    }
}

/// One shard's whole flight, decided from a handful of numbers before it is
/// drawn even once.
///
/// A pure function, for the same reason [`crate::effects::gesture_shot`] is one:
/// the shape of a burst has to be assertable without a window, a frame or a
/// random number generator. Every random choice is passed in, so a test can pin
/// `t` and read the trajectory straight off.
fn shot(x: f32, y: f32, t: f32, piece: Piece, shape: usize, color: [u8; 3]) -> Shard {
    // `t` walks a full turn from the shard's own centre, so the pieces leave in
    // every direction rather than in a line, and because it is an argument
    // rather than a random draw the same `t` always gives the same angle.
    let angle = t * std::f32::consts::TAU;
    let speed = SPEED_MIN + (SPEED_MAX - SPEED_MIN) * t;
    Shard {
        x,
        y,
        vx: angle.cos() * speed,
        vy: angle.sin() * speed,
        rot: t * 360.0,
        spin: if t < 0.5 {
            -(SPIN_MIN + (SPIN_MAX - SPIN_MIN) * t * 2.0)
        } else {
            SPIN_MIN + (SPIN_MAX - SPIN_MIN) * (t * 2.0 - 1.0)
        },
        piece,
        shape,
        color,
        age: 0.0,
        life: LIFE_MIN + (LIFE_MAX - LIFE_MIN) * t,
    }
}

/// Where a burst is thrown from, as a fraction of the layout, for a `t`.
///
/// The middle of the screen, so the pieces radiate outwards over the well *and*
/// the side panel rather than raining in one column. Returns `(x, y)` in layout
/// pixels. Pure, for the same reason [`shot`] is.
fn origin_for(t: f32) -> (f32, f32) {
    // Pushed off the exact centre: a burst thrown from dead centre leaves a hole
    // in the middle of the screen, which reads as the explosion having missed.
    let r = 0.12 + 0.10 * (1.0 - (t - 0.5).abs() * 2.0);
    let angle = t * std::f32::consts::TAU * 1.7;
    (
        CONTENT_WIDTH as f32 * (0.5 + r * angle.cos()),
        SCREEN_HEIGHT as f32 * (0.5 + r * angle.sin()),
    )
}

/// The new-record celebration.
#[derive(Default)]
pub struct Celebration {
    shards: Vec<Shard>,
    /// Wall-clock milliseconds the celebration stops at. Zero when it is not
    /// running, which is why [`Default`] is correct rather than merely
    /// convenient.
    until: u64,
    /// Fraction of the next shard still owed, carried between frames so the
    /// rate is per second and not per frame. Without it the burst density would
    /// silently double on a machine running at 120 Hz.
    owed: f32,
}

impl Celebration {
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin a celebration at `now`, in milliseconds.
    ///
    /// Called on the frame a run is recognised as beating the record, which is
    /// the same frame the player is sent to the initials screen. Restarting
    /// rather than extending is deliberate: a record can only be beaten once in
    /// a run, and a second call means something changed the player's mind, in
    /// which case the old shards in the air are stale and go with it.
    pub fn start(&mut self, now: u64) {
        self.shards.clear();
        self.until = now + HIGH_SCORE_CELEBRATION_MS;
        self.owed = 0.0;
    }

    /// Whether anything is being thrown, or in the air, at `now`.
    pub fn running(&self, now: u64) -> bool {
        now < self.until
    }

    /// Stop immediately, keeping whatever is already in the air.
    ///
    /// For leaving the record screen: the shards do not belong to the next run,
    /// and dropping them mid-flight looks worse than letting them land.
    pub fn stop(&mut self) {
        self.until = 0;
        self.owed = 0.0;
    }

    /// Throw what is owed and move everything already in the air.
    pub fn update(&mut self, dt: f32, now: u64) {
        if self.running(now) {
            self.owed += RATE * dt;
            let mut rng = rand::thread_rng();
            while self.owed >= 1.0 {
                self.owed -= 1.0;
                let t = rng.gen::<f32>();
                let (x, y) = origin_for(t);
                let piece = ALL_PIECES[rng.gen_range(0..ALL_PIECES.len())];
                // Any of the four rotation states. All seven pieces have four,
                // the O's four being identical, so this is a free extra axis of
                // variety for six of them.
                let shape = rng.gen_range(0..4);
                // Index 1-7 of `COLORS`, matching the well: index 0 is the
                // "empty" sentinel and would throw a black tetromino, which is
                // the one colour guaranteed to be invisible against the
                // backdrop.
                let color = COLORS[1 + rng.gen_range(0..7)];
                self.shards.push(shot(x, y, t, piece, shape, color));
            }
        } else {
            // Window closed: stop accruing a debt, and pay off the one that was
            // in hand, so a frame that lands either side of the deadline does
            // not change how many shards a player sees.
            self.owed = 0.0;
        }
        for s in &mut self.shards {
            s.age += dt;
            s.x += s.vx * dt;
            s.y += s.vy * dt;
            s.vy += FALL * dt;
            s.rot += s.spin * dt;
        }
        self.shards.retain(|s| s.age < s.life);
    }

    pub fn draw(&self, d: &mut RaylibDrawHandle) {
        for s in &self.shards {
            s.draw(d);
        }
    }

    /// How many shards are in the air. For the tests, which cannot see them.
    #[cfg(test)]
    fn airborne(&self) -> usize {
        self.shards.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HIGH_SCORE_CELEBRATION_MS as DURATION;

    /// Milliseconds in one [`DT`], for driving the clock in the tests.
    #[cfg(test)]
    const FRAME_MS: u64 = (1.0 / 60.0 * 1000.0) as u64;

    const DT: f32 = 1.0 / 60.0;

    /// A celebration that has not been asked for throws nothing and draws
    /// nothing.
    ///
    /// The load-bearing property of the whole type: a new [`Celebration`] is
    /// created once at startup and lives for the entire session, and this module
    /// is reached from the game-over path of every single run. If it threw
    /// anything without [`Celebration::start`] then every top-out in the game
    /// would set off a party.
    #[test]
    fn nothing_happens_until_a_record_is_set() {
        let mut c = Celebration::new();
        assert!(!c.running(0));
        for _ in 0..300 {
            c.update(DT, 0);
        }
        assert_eq!(c.airborne(), 0, "shards were thrown with no record set");
    }

    /// Beating the record throws pieces, and keeps throwing for its whole window
    /// rather than in one initial burst.
    #[test]
    fn a_record_throws_pieces_for_the_whole_window() {
        let mut c = Celebration::new();
        c.start(0);
        assert!(c.running(1));
        // Stepped frame by frame, because that is the only way the count means
        // anything. Handing `update` a whole second of `dt` throws eighteen
        // shards *and then ages all eighteen by a second in the same call, so
        // most of them die on the frame they were born and the screen comes out
        // empty. That is correct behaviour for a one second frame and a broken
        // way to test a burst.
        let mut early = 0;
        for frame in 0..90u64 {
            c.update(DT, frame * FRAME_MS);
            if frame == 45 {
                early = c.airborne();
            }
        }
        assert!(
            early >= 8,
            "only {early} shard(s) in flight a quarter of the way in"
        );
        // And it must not have been all thrown at once. Compared as a fraction
        // rather than as `>=`: the count is the product of a constant rate and
        // the spread of shard lifetimes, so it sits near a steady state and
        // wobbles either side of it. What has to be ruled out is the count
        // *collapsing* - a burst that finished in the first second is not a
        // three second celebration - and a ratio catches that while a `>=` would
        // fail on an unlucky wobble and pass a burst that stopped just as often.
        for frame in 90..165u64 {
            c.update(DT, frame * FRAME_MS);
        }
        let late = c.airborne();
        assert!(
            late as f32 >= early as f32 * 0.5,
            "{late} shard(s) in flight at three quarters of the window against \
             {early} at a quarter, so the burst is finishing early"
        );
        assert!(
            c.running(DURATION - 1),
            "the window closed early: {}ms of simulated time was already past the \
             {DURATION}ms deadline",
            165 * FRAME_MS
        );
    }

    /// Three seconds, then it stops: no new shards, and the ones in the air are
    /// gone inside one more shard-life so the screen is not left littered.
    #[test]
    fn it_stops_after_its_window() {
        let mut c = Celebration::new();
        c.start(0);
        for _ in 0..60 * 3 {
            c.update(DT, 0);
        }
        let at_deadline = c.airborne();
        assert!(
            !c.running(DURATION),
            "still running at the {DURATION}ms deadline"
        );
        // Nothing new after the deadline, and the stragglers land.
        for step in 0..180u64 {
            c.update(DT, DURATION + step * FRAME_MS);
        }
        assert_eq!(
            c.airborne(),
            0,
            "{at_deadline} shard(s) were still in the air long after the window"
        );
    }

    /// Leaving the record screen stops the throwing at once.
    #[test]
    fn stopping_halts_it() {
        let mut c = Celebration::new();
        c.start(0);
        for _ in 0..10 {
            c.update(DT, 0);
        }
        assert!(c.airborne() > 0);
        c.stop();
        assert!(!c.running(1));
        // The ones already in the air are kept, so they can land, but nothing
        // new comes out. A second of frames either side of the stop must not
        // change the count.
        let held = c.airborne();
        for frame in 0..60u64 {
            c.update(DT, frame * FRAME_MS);
        }
        assert!(
            c.airborne() <= held,
            "shards kept being thrown after stop(): {} in the air against {held} \
             held",
            c.airborne()
        );
    }

    /// Setting a record twice does not stack two celebrations into one.
    #[test]
    fn starting_again_does_not_stack() {
        let mut c = Celebration::new();
        c.start(0);
        c.update(DT, 0);
        c.start(FRAME_MS);
        assert!(
            c.airborne() <= 1,
            "a second start left {} shards from the first one behind",
            c.airborne()
        );
        assert!(
            c.running(DURATION),
            "the window was measured from the first start, not the second"
        );
    }

    /// The pieces have to land on *both* halves of the screen.
    ///
    /// "Around the UI" is the whole request, and the layout is not square: the
    /// side panel is 369 of the 819 pixels. A burst that only ever appeared left
    /// of [`crate::config::SCREEN_WIDTH`] would be a normal-looking explosion
    /// over the well and would miss the point of it.
    #[test]
    fn the_burst_reaches_the_panel_as_well_as_the_well() {
        let mut over_well = 0;
        let mut over_panel = 0;
        // Sampled straight from the origin function rather than from a live run,
        // so this cannot pass or fail on how the random draws happened to fall.
        for step in 0..2000 {
            let t = step as f32 / 2000.0;
            let (x, _) = origin_for(t);
            if x < crate::config::SCREEN_WIDTH as f32 {
                over_well += 1;
            } else {
                over_panel += 1;
            }
        }
        assert!(
            over_well > 0 && over_panel > 0,
            "{over_well} origin(s) over the well and {over_panel} over the panel, so \
             the burst only ever covers one of them"
        );
        // Both halves, and neither one barely touched: a burst that grazed the
        // panel would technically pass the test above.
        assert!(
            over_panel as f64 / 2000.0 > 0.2,
            "only {:.0}% of origins are over the panel",
            over_panel as f64 / 20.0
        );
    }

    /// A shard is a tetromino: four blocks, all distinct, all within its own
    /// bounding box plus half a cell of rotation slack.
    #[test]
    fn every_shard_is_a_real_tetromino() {
        for piece in ALL_PIECES {
            for shape in 0..4 {
                let s = shot(100.0, 100.0, 0.37, piece, shape, COLORS[1]);
                let cells = s.cells();
                for i in 0..4 {
                    for j in i + 1..4 {
                        assert!(
                            (cells[i].0 - cells[j].0).abs() > 0.001
                                || (cells[i].1 - cells[j].1).abs() > 0.001,
                            "{piece:?} in state {shape} put two blocks on top of each \
                             other"
                        );
                    }
                }
                // A block is one cell wide, so two of them can never be more than
                // three cells apart centre to centre along either axis, and the
                // furthest corner out from the centre is a block and a half.
                for (cx, cy) in cells {
                    assert!(
                        (cx - 100.0).abs() <= SHARD_CELL * 1.5 + 0.01
                            && (cy - 100.0).abs() <= SHARD_CELL * 1.5 + 0.01,
                        "{piece:?} in state {shape} put a block at ({cx}, {cy}), \
                         outside its own bounding box"
                    );
                }
            }
        }
    }

    /// The shards are thrown outwards in every direction, not in a line.
    #[test]
    fn the_burst_goes_in_every_direction() {
        let mut right = 0;
        let mut left = 0;
        let mut up = 0;
        let mut down = 0;
        for step in 0..2000 {
            let t = step as f32 / 2000.0;
            let s = shot(0.0, 0.0, t, Piece::T, 0, COLORS[1]);
            if s.vx.abs() > s.vy.abs() {
                if s.vx > 0.0 { right += 1 } else { left += 1 }
            } else if s.vy > 0.0 {
                down += 1;
            } else {
                up += 1;
            }
        }
        for (n, name) in [(right, "right"), (left, "left"), (up, "up"), (down, "down")] {
            assert!(n > 200, "only {n} shards went {name} of 2000");
        }
    }

    /// Debris falls, and comes back down when it is thrown up.
    ///
    /// A celebration whose pieces hang in the air reads as a freeze-frame rather
    /// than an explosion. Two halves, because the burst throws in every
    /// direction: gravity has to accelerate a shard downwards, and a shard
    /// launched upwards has to turn around and come back rather than sail off
    /// the top of the screen.
    #[test]
    fn a_shard_falls_and_comes_back_down() {
        // Gravity, on its own: every frame adds the same amount to `vy`, so
        // after thirty frames it must be `30 * FALL * DT` higher than it started,
        // whatever direction the shard was thrown.
        for step in 0..200 {
            let t = step as f32 / 200.0;
            let mut s = shot(0.0, 0.0, t, Piece::L, 1, COLORS[1]);
            let vy0 = s.vy;
            for _ in 0..30 {
                s.y += s.vy * DT;
                s.vy += FALL * DT;
            }
            assert!(
                (s.vy - vy0 - 30.0 * FALL * DT).abs() < 0.05,
                "at t={t} gravity added {} to vy, not {}",
                s.vy - vy0,
                30.0 * FALL * DT
            );
        }

        // Thrown straight up. `t` three quarters of a turn puts the angle at
        // 270 degrees, where `sin` is -1, and y grows downwards, so -1 is up.
        let mut up = shot(0.0, 0.0, 0.75, Piece::I, 0, COLORS[1]);
        let launch = up.vy;
        assert!(launch < -200.0, "the shard was not thrown upwards: {launch}");

        // Climb to the apex, remembering how high it got.
        let mut apex = up.y;
        for _ in 0..120 {
            up.y += up.vy * DT;
            up.vy += FALL * DT;
            apex = apex.min(up.y);
        }
        // Then let it come back down and check it ends up below where it
        // started, rather than sailing off the top of the screen.
        let mut landed = up.y;
        for _ in 0..120 {
            up.y += up.vy * DT;
            up.vy += FALL * DT;
            landed = landed.max(up.y);
        }
        assert!(
            apex < 0.0,
            "the shard thrown up at {launch}px/s never rose above where it started"
        );
        assert!(
            landed > apex + 10.0,
            "the shard rose to {apex} and was still at {landed} 240 frames later, \
             so it never comes back down"
        );
    }

    /// A shard fades in as well as out, and is gone when its life ends.
    ///
    /// The fade-in is the half that is easy to leave out: a burst throws several
    /// shards a frame, so without it every one of them appears at full strength
    /// and the burst pops rather than blooms.
    #[test]
    fn a_shard_fades_in_and_out() {
        let mut s = shot(0.0, 0.0, 0.5, Piece::O, 0, COLORS[1]);
        assert_eq!(s.alpha(), 0, "a brand new shard is at full strength");
        s.age = s.life * 0.05;
        let early = s.alpha();
        s.age = s.life * 0.5;
        let mid = s.alpha();
        s.age = s.life * 0.95;
        let late = s.alpha();
        s.age = s.life;
        let over = s.alpha();
        assert!(early > 0, "the fade-in never gets going");
        assert!(early < mid, "the fade-in never arrives: {early} then {mid}");
        assert!(mid > late, "the shard never fades out: {mid} then {late}");
        assert!(
            late < 32,
            "the shard is still at {late}/255 with only 5% of its life left"
        );
        assert_eq!(over, 0, "the shard is still drawn at the end of its life");
        // And it must reach full strength, or the whole burst is dimmer than it
        // was meant to be.
        assert!(
            mid >= 250,
            "the brightest moment of a shard's life is only {mid}/255"
        );
        // Stated directly as well, because the assertion above only catches the
        // overlap by its effect. The two ramps have to leave a gap between them,
        // and that gap is the flat full-strength stretch; retune either constant
        // past the other and the peak quietly drops, which is invisible here and
        // very visible on screen.
        assert!(
            FADE_IN + FADE_OUT <= 1.0,
            "the ramps overlap: {} of fade in plus {} of fade out leaves no time at \
             full strength",
            FADE_IN,
            FADE_OUT
        );
    }

    /// The throw rate is per second, so a fast machine does not throw twice as
    /// many pieces as a slow one.
    #[test]
    fn the_rate_is_per_second_and_not_per_frame() {
        let at_60 = {
            let mut c = Celebration::new();
            c.start(0);
            for _ in 0..60 {
                c.update(DT, 0);
            }
            c.airborne()
        };
        let at_144 = {
            let mut c = Celebration::new();
            c.start(0);
            let dt = 1.0 / 144.0;
            for _ in 0..144 {
                c.update(dt, 0);
            }
            // Normalise: at 144 Hz every shard is younger, so more are still
            // alive. What has to match is how many were *thrown*, which is the
            // count the frames add up to over the same second.
            c.airborne()
        };
        // Both are a little over RATE because the oldest are still in the air;
        // what must not happen is 144 Hz putting 2.4x as many on screen.
        assert!(
            (at_60 as f32 / at_144 as f32) > 0.7 && (at_60 as f32 / at_144 as f32) < 1.4,
            "60 Hz threw {at_60} against 144 Hz's {at_144}, so the rate is per frame"
        );
    }
}
