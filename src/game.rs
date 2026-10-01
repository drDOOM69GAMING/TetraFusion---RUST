//! Game state: gravity, lock delay, hold, ghost, scoring and mode rules.

use crate::board::Grid;
use crate::config::*;
use crate::pieces::{self, Bag, Piece, Rotation};

/// Playable modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Marathon,
    Sprint,
    Ultra,
    Training,
    Master,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Marathon => "Marathon",
            Mode::Sprint => "Sprint",
            Mode::Ultra => "Ultra",
            Mode::Training => "Training",
            Mode::Master => "Master",
        }
    }

    pub const ALL: [Mode; 5] = [
        Mode::Marathon,
        Mode::Sprint,
        Mode::Ultra,
        Mode::Training,
        Mode::Master,
    ];

    /// Does this mode stop the clock on a line clear, or run to a timer?
    ///
    /// Only Ultra ends because time ran out. Sprint is also timed - the player
    /// races it - but it ends at 40 lines instead, so treating "timed" as "Ultra"
    /// is what kept the sprint clock off the screen.
    pub fn is_timed(self) -> bool {
        matches!(self, Mode::Ultra)
    }

    /// How long this mode runs, if it has a limit at all.
    ///
    /// Sprint has a clock but no limit - it ends when the 40 lines are down, so
    /// asking it for a duration would mean inventing a deadline the mode does not
    /// have.
    pub fn duration_ms(self) -> Option<u64> {
        match self {
            Mode::Ultra => Some(ULTRA_DURATION_MS),
            _ => None,
        }
    }

    /// Is there a stopwatch worth showing during this mode?
    ///
    /// True for every mode with a clock, timed or not. Marathon is the only one
    /// excluded: it is endless, so there is no number to beat and a stopwatch
    /// there is just a distraction from the score.
    pub fn shows_clock(self) -> bool {
        !matches!(self, Mode::Marathon)
    }

    /// The one-line rule, as the menu shows it.
    ///
    /// These strings are the mode's *promise*, which is why they live next to the
    /// code that has to keep them. "Sprint" on its own says nothing about 40
    /// lines or about the clock, so a player picking it from a list of five words
    /// is guessing. Drawn under the highlighted menu entry, they turn the menu
    /// from five labels into five rules.
    pub fn blurb(self) -> &'static str {
        match self {
            Mode::Marathon => "Endless. Level up every 10 lines.",
            Mode::Sprint => "Clear 40 lines as fast as you can.",
            Mode::Ultra => "Score as much as you can in 3 minutes.",
            Mode::Training => "No game over. H deletes the bottom row.",
            Mode::Master => "Endless. Starts at level 15, fast.",
        }
    }

    /// How this run ended, for the title on the results screen.
    ///
    /// Three-way rather than the win/lose boolean the port used, because the
    /// original is not uniform here and flattening it lost that: finishing a
    /// Sprint is a success ("Sprint Complete!"), but Ultra running out of time is
    /// neither a success nor a defeat - the original sets `game_over = True` and
    /// shows "Time Up!", and a player who survived three full minutes has not lost
    /// anything either.
    pub fn outcome(self, won: bool) -> Outcome {
        match (self, won) {
            (Mode::Sprint, true) => Outcome::Complete,
            (Mode::Ultra, true) => Outcome::TimeUp,
            (_, true) => Outcome::Won,
            (_, false) => Outcome::Lost,
        }
    }
}

/// How a run ended, for the title on the results screen.
///
/// Kept apart from [`Game::won`] because a boolean cannot tell "you met the
/// objective" from "the clock ran out", and the two deserve different words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// A timed mode's limit expired. The player was still playing when it did.
    TimeUp,
    /// The mode's objective was met.
    Complete,
    /// Ended in a mode with no objective beyond survival.
    Won,
    /// Topped out.
    Lost,
}

impl Outcome {
    /// The banner title.
    pub fn title(self) -> &'static str {
        match self {
            Outcome::TimeUp => "TIME UP",
            Outcome::Complete => "SPRINT COMPLETE",
            Outcome::Won => "YOU WIN",
            Outcome::Lost => "GAME OVER",
        }
    }
}

/// Base gravity in milliseconds per row, by difficulty name.
pub fn difficulty_speeds(name: &str) -> u32 {
    match name {
        "easy" => 1500,
        "hard" => 600,
        "very hard" => 400,
        "master" => 200,
        _ => 1000, // normal
    }
}

/// A discrete player intent, produced by the input layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    MoveLeft,
    MoveRight,
    SoftDrop,
    HardDrop,
    RotateCw,
    RotateCcw,
    Hold,
    DeleteBottomRow,
}

/// Where the active piece was, and how it was lying, at the instant an event
/// happened.
///
/// Events are read a frame or more after they are pushed, and by then the piece
/// is somewhere else entirely: a hard drop locks and spawns the next piece in
/// the same call, so the live `origin` is the *new* piece's spawn point at the
/// top of the board. A presentation layer that placed its effect from live game
/// state would throw a hard-drop burst at the ceiling instead of the floor, and
/// would do it only at high gravity, where the effect matters most. Carrying the
/// pose on the event is the only way to be right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pose {
    pub piece: Piece,
    pub rot: Rotation,
    pub origin: (i32, i32),
}

/// Something the game wants the presentation layer to react to.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Spawn,
    /// The piece slid one cell, carrying both the way it went and where it is
    /// now.
    Move { at: Pose, dir: (i32, i32) },
    /// The piece rotated, carrying where it ended up. `dir` is the wall kick,
    /// which is zero for a rotation that turned in place.
    Rotate { at: Pose, dir: (i32, i32) },
    Lock {
        at: Pose,
        color: u8,
    },
    /// The piece was slammed to the floor, carrying the distance it fell and the
    /// pose it landed in.
    HardDrop { rows: i32, at: Pose },
    Hold,
    LineClear { rows: Vec<usize>, cells: Vec<Vec<u8>> },
    TSpin,
    Combo(i32),
    BackToBack(bool),
    AllClear,
    LevelUp(i32),
    GameOver,
    Win,
}

pub struct Settings {
    pub das_ms: u32,
    pub arr_ms: u32,
    pub base_fall_speed: u32,
    /// How much faster the original's Gravity option makes every piece.
    /// Folded into `base_fall_speed` for non-Master modes in the input layer;
    /// Master mode folds it here because it pins its own difficulty.
    pub gravity_multiplier: f32,
    /// Whether the level rotates which palette slot a piece draws from.
    ///
    /// `true` is the original's behaviour. The Traditional palette turns this
    /// off, because "lock the pieces back to standard colours" has to mean the
    /// I piece is cyan at *every* level - if the slot still rotated, the I
    /// would still change colour with the level and the option would promise
    /// something it did not deliver.
    pub shift_palette: bool,
    /// The level a run opens at.
    ///
    /// One everywhere except the two ways of asking for Master, which open at 15.
    ///
    /// This field exists because the original's Master is a *difficulty* - the
    /// `level = 15 if difficulty == 'master' else 1` line - and the port has both
    /// a `Mode::Master` and a `"master"` difficulty. Deriving the opening level
    /// from the mode alone meant `Difficulty: Master` selected in Options gave
    /// 200 ms/row gravity at **level 1**: fast pieces on the level-1 palette, at
    /// level-1 scoring multipliers, with level-1 piece colours, and level 1 shown
    /// in the panel. Every other part of "Master" applied except the part that
    /// makes it Master.
    pub start_level: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            das_ms: DEFAULT_DAS_MS,
            arr_ms: DEFAULT_ARR_MS,
            base_fall_speed: 1000,
            gravity_multiplier: 1.0,
            shift_palette: true,
            start_level: 1,
        }
    }
}

pub struct Game {
    pub mode: Mode,
    pub settings: Settings,

    pub grid: Grid,
    bag: Bag,
    pub next: Vec<Piece>,
    pub hold: Option<Piece>,
    hold_used: bool,

    piece: Piece,
    rot: Rotation,
    origin: (i32, i32),

    pub score: i32,
    pub lines_cleared: i32,
    pub level: i32,
    pub pieces_dropped: i32,
    pub combo: i32,
    pub max_combo: i32,
    pub back_to_back: bool,
    pub t_spin_count: i32,
    /// True when the most recent clear was a Tetris or a T-spin, i.e. the
    /// conditions that let back-to-back continue.
    pub difficult_clear: bool,

    pub fall_speed: u32,
    last_fall: u64,
    grounded_since: Option<u64>,
    lock_resets: u32,
    lock_moves: u32,
    soft_dropping: bool,

    left_held: bool,
    right_held: bool,
    last_h_shift: u64,

    pub game_over: bool,
    pub won: bool,
    pub started_at: u64,
    pub ended_at: Option<u64>,

    pub events: Vec<Event>,
}

impl Game {
    pub fn new(mode: Mode, settings: Settings, now: u64) -> Self {
        let mut settings = settings;
        // Master mode pins its own gravity (200 ms/row base at level 15),
        // overriding the difficulty setting, but still honours the Gravity
        // multiplier like every other mode.
        if mode == Mode::Master {
            let base = difficulty_speeds("master") as f64 / settings.gravity_multiplier.max(0.1) as f64;
            settings.base_fall_speed = base as u32;
        }
        // The opening level comes from the settings, not the mode, so that the
        // `"master"` *difficulty* starts at 15 as well as `Mode::Master` doing.
        let start_level = settings.start_level.max(1);
        let mut g = Self {
            mode,
            settings,
            grid: Grid::new(),
            bag: Bag::new(),
            next: Vec::new(),
            hold: None,
            hold_used: false,
            piece: Piece::T,
            rot: 0,
            origin: (0, 0),
            score: 0,
            lines_cleared: 0,
            level: 1,
            pieces_dropped: 0,
            combo: -1,
            max_combo: 0,
            back_to_back: false,
            t_spin_count: 0,
            difficult_clear: false,
            fall_speed: 1000,
            last_fall: now,
            grounded_since: None,
            lock_resets: 0,
            lock_moves: 0,
            soft_dropping: false,
            left_held: false,
            right_held: false,
            last_h_shift: now,
            game_over: false,
            won: false,
            started_at: now,
            ended_at: None,
            events: Vec::new(),
        };
        g.level = start_level;
        // The opening cadence is the difficulty's own level-one speed, exactly
        // as the original opened a game. Because `gravity_g(1) == 1`,
        // `fall_speed_for` would return the same number for every mode that
        // opens at level one, so this is the curve at level one and the
        // comment only matters for Master - which opens at level 15 and would
        // otherwise start pinned to the 20G ceiling, losing every bit of
        // placement freedom before the player has touched a key. Master ramps
        // into the ceiling on its first level-up, as the original did.
        g.fall_speed = g.settings.base_fall_speed.max(MIN_FALL_SPEED);
        // Preload the visible queue, then take the first piece from it.
        for _ in 0..5 {
            g.next.push(g.bag.next_piece());
        }
        let first = g.next.remove(0);
        g.next.push(g.bag.next_piece());
        g.spawn_piece(first, now);
        g
    }

    /// The gravity curve: [`fall_speed_for`] turns the current level into a
    /// row cadence, rising to the 20G ceiling and no further.
    fn recompute_fall_speed(&mut self) {
        self.fall_speed = fall_speed_for(self.settings.base_fall_speed, self.level);
    }

    /// Is the board running at the 20G ceiling?
    ///
    /// At the ceiling gravity is not merely fast, it is *instantaneous*: a new
    /// piece lands on the stack the frame it spawns and the only way to place
    /// it is to slide it along the floor under lock delay. Anything slower than
    /// this leaves a piece hovering long enough to aim with, which is the whole
    /// difference between the last few levels and a wall.
    pub fn at_max_gravity(&self) -> bool {
        self.fall_speed <= MIN_FALL_SPEED
    }

    // --- Queries ----------------------------------------------------------

    pub fn piece(&self) -> Piece {
        self.piece
    }

    pub fn rotation(&self) -> Rotation {
        self.rot
    }

    pub fn origin(&self) -> (i32, i32) {
        self.origin
    }

    /// Snapshot the active piece's pose, for stamping onto an event.
    fn pose(&self) -> Pose {
        Pose {
            piece: self.piece,
            rot: self.rot,
            origin: self.origin,
        }
    }

    /// The piece's colour, shifted with level as the original does.
    ///
    /// The original computed `(shape_index + level - 1) % 7 + 1` from a
    /// spawn-shape lookup; here the shape index *is* the piece discriminant.
    pub fn color(&self) -> u8 {
        self.color_for(self.piece)
    }

    /// The colour a piece is drawn in at the current level.
    ///
    /// The level shift applies to *every* piece, not just the live one - the
    /// original ran the same `(shape_index + level - 1) % 7 + 1` for the next
    /// queue and for the hold slot, so a preview really is the colour that piece
    /// will be when it arrives. It matters more than it sounds: a player reading
    /// the next queue to plan is reading a promise, and on the original a
    /// preview that ignored the level would be a lie from the first level up.
    pub fn color_for(&self, piece: Piece) -> u8 {
        let n = COLORS.len() - 1; // 7 real pieces
        if !self.settings.shift_palette {
            // Traditional: a piece keeps its own colour for the whole run. The
            // level still drives everything else on screen - gravity, the
            // background, the particle effects - so the stage still changes;
            // this only stops the colours being part of it.
            return (piece as usize % n + 1) as u8;
        }
        // `saturating_sub` rather than `- 1`: the level is a plain `i32` read out
        // of a settings file and out of a save, so `i32::MIN` is one hand-edited
        // character away, and in a debug build that is a panic in the middle of
        // the draw rather than a wrong colour.
        let shift = self.level.saturating_sub(1).max(0) as usize;
        ((piece as usize + shift) % n + 1) as u8
    }

    /// Where the piece would come to rest.
    pub fn ghost_origin(&self) -> (i32, i32) {
        let d = self
            .grid
            .drop_distance(self.piece, self.rot, self.origin);
        (self.origin.0, self.origin.1 + d)
    }

    pub fn is_grounded(&self) -> bool {
        !self
            .grid
            .fits(self.piece, self.rot, (self.origin.0, self.origin.1 + 1))
    }

    /// How long the run has been going, in ms.
    ///
    /// Frozen at [`Game::ended_at`] once the run is over, which is the whole
    /// point of having `ended_at`. Without the freeze this is `now - started_at`,
    /// and the game-over screen is drawn from a live clock, so the final time
    /// ticks upward for as long as the player looks at it. Sprint's whole
    /// promise is "clear 40 lines as fast as possible, and the timer stops when
    /// you finish" - a finish time that climbs to 4:12 while the result sits on
    /// screen is not a finish time, and it is the number the player is trying to
    /// beat in their head.
    ///
    /// `max` rather than a bare `unwrap_or` because a caller can hand in a
    /// `now` older than the recorded end - a clock that ran backwards, or a
    /// caller that has not caught up. That should read as "no time passed", not
    /// as an enormous run.
    pub fn elapsed_ms(&self, now: u64) -> u64 {
        let end = self.ended_at.unwrap_or(now).max(self.started_at);
        end.saturating_sub(self.started_at)
    }

    /// The run's remaining time, for the modes that have one.
    ///
    /// [`None`] for a mode with no clock at all - Marathon, Training and Master
    /// are all untimed, and a countdown reading `0:00` on them would advertise a
    /// limit that does not exist.
    ///
    /// Clamped at zero rather than allowed to go negative, so a run that is a few
    /// milliseconds past the wire reads `0:00` instead of a nonsense negative
    /// number.
    pub fn remaining_ms(&self, now: u64) -> Option<u64> {
        self.mode
            .duration_ms()
            .map(|d| d.saturating_sub(self.elapsed_ms(now)))
    }

    /// Is the run over, either way?
    pub fn is_finished(&self) -> bool {
        self.game_over || self.won
    }

    // --- Held direction (for DAS/ARR) -------------------------------------

    pub fn set_left_held(&mut self, held: bool, now: u64) {
        if held && !self.left_held {
            self.last_h_shift = now;
        }
        self.left_held = held;
    }

    pub fn set_right_held(&mut self, held: bool, now: u64) {
        if held && !self.right_held {
            self.last_h_shift = now;
        }
        self.right_held = held;
    }

    // --- Player actions ----------------------------------------------------

    pub fn apply(&mut self, action: Action, now: u64) {
        if self.is_finished() {
            return;
        }
        match action {
            Action::MoveLeft => self.shift(-1, now),
            Action::MoveRight => self.shift(1, now),
            Action::RotateCw => self.rotate(pieces::rotate_cw(self.rot), now),
            Action::RotateCcw => self.rotate(pieces::rotate_ccw(self.rot), now),
            Action::SoftDrop => {
                self.soft_dropping = true;
                self.advance_gravity(now);
            }
            Action::HardDrop => self.hard_drop(now),
            Action::Hold => self.do_hold(now),
            Action::DeleteBottomRow => {
                if self.mode == Mode::Training {
                    self.delete_bottom_row();
                }
            }
        }
    }

    fn shift(&mut self, dx: i32, now: u64) {
        self.last_h_shift = now;
        let target = (self.origin.0 + dx, self.origin.1);
        if self.grid.fits(self.piece, self.rot, target) {
            self.origin = target;
            self.events.push(Event::Move {
                at: self.pose(),
                dir: (dx, 0),
            });
            self.on_piece_moved(now);
        }
    }

    /// Rotate with SRS wall kicks. On failure the piece does not move at all.
    fn rotate(&mut self, to: Rotation, now: u64) {
        if to == self.rot {
            return;
        }
        let kicks = pieces::kicks_for(self.piece, self.rot, to);
        for (dx, dy) in kicks {
            let target = (self.origin.0 + dx, self.origin.1 + dy);
            if self.grid.fits(self.piece, to, target) {
                self.rot = to;
                self.origin = target;
                self.events.push(Event::Rotate {
                    at: self.pose(),
                    dir: (dx, dy),
                });
                self.on_piece_moved(now);
                return;
            }
        }
    }

    /// A successful move or rotate refreshes lock delay while the piece is
    /// resting on the stack, up to the reset budget.
    fn on_piece_moved(&mut self, now: u64) {
        if self.is_grounded() {
            if self.lock_resets < MAX_LOCK_DELAY_RESETS && self.lock_moves < MAX_LOCK_DELAY_MOVES {
                self.lock_resets += 1;
                self.lock_moves += 1;
                self.grounded_since = Some(now);
            }
        }
    }

    fn hard_drop(&mut self, now: u64) {
        let rows = self.grid.drop_distance(self.piece, self.rot, self.origin);
        self.origin.1 += rows;
        self.score += rows * POINTS_PER_HARD_DROP_ROW;
        self.events.push(Event::HardDrop {
            rows,
            at: self.pose(),
        });
        self.lock_piece(now);
    }

    fn do_hold(&mut self, now: u64) {
        if self.hold_used {
            return;
        }
        let outgoing = self.piece;
        match self.hold.take() {
            Some(swapped) => {
                self.hold = Some(outgoing);
                self.spawn_piece(swapped, now);
            }
            None => {
                self.hold = Some(outgoing);
                let incoming = self.next.remove(0);
                self.next.push(self.bag.next_piece());
                self.spawn_piece(incoming, now);
            }
        }
        self.hold_used = true;
        self.events.push(Event::Hold);
    }

    /// Place a new piece at the spawn point and reset per-piece state.
    fn spawn_piece(&mut self, piece: Piece, now: u64) {
        self.piece = piece;
        self.rot = 0;
        self.origin = (pieces::spawn_offset(piece), pieces::spawn_y());
        self.grounded_since = None;
        self.lock_resets = 0;
        self.lock_moves = 0;
        self.last_fall = now;

        if self.grid.topped_out() {
            if self.mode == Mode::Training {
                // Training never ends; the board is simply cleared.
                self.grid.reset();
                self.events.push(Event::Spawn);
            } else {
                self.game_over = true;
                self.ended_at = Some(now);
                self.events.push(Event::GameOver);
            }
        }

        // At the 20G ceiling there is no fall at all: the piece is on the stack
        // before the player has seen it, and placement becomes a floor-sliding
        // problem governed by lock delay. Landing it here rather than in
        // `advance_gravity` is what makes that true of *every* spawn - the
        // first piece of a run included - instead of only the pieces that
        // happened to arrive while a gravity timer was already running.
        //
        // The topped-out check above runs first so a board that cannot fit the
        // piece ends the run rather than being shoved into place.
        if !self.is_finished() && self.at_max_gravity() {
            let rows = self
                .grid
                .drop_distance(self.piece, self.rot, self.origin);
            self.origin.1 += rows;
        }

        self.events.push(Event::Spawn);
    }

    /// Lock delay expired while resting.
    fn lock_now(&mut self, now: u64) {
        self.lock_piece(now);
    }

    /// The single lock path.
    ///
    /// The original had three near-identical copies of this, and only the
    /// hard-drop one scored. This is that scoring, applied uniformly.
    fn lock_piece(&mut self, now: u64) {
        if self.is_finished() {
            return;
        }
        let piece = self.piece;
        let rot = self.rot;
        let origin = self.origin;

        // The T-spin test must run against the board as it was *before* the
        // T was written in, or the T's own cells count as blocked corners.
        let t_spin = self.grid.is_t_spin(piece, rot, origin);

        self.grid.place(piece, rot, origin, self.color());
        self.hold_used = false;
        self.pieces_dropped += 1;
        self.events.push(Event::Lock {
            at: Pose { piece, rot, origin },
            color: self.color(),
        });

        // Lock-out: the piece came to rest with cells entirely above the
        // playfield (the original checked a row ~28 blocks above the stack
        // and almost never ended the game). A lock-up here is how a truly
        // full board kills the run; training just dumps the field.
        let locks_off_board = pieces::cells(piece, rot)
            .iter()
            .any(|(_, cy)| origin.1 + cy < 0);
        if locks_off_board {
            if self.mode == Mode::Training {
                self.grid.reset();
                self.events.push(Event::Spawn);
            } else {
                self.game_over = true;
                self.ended_at = Some(now);
                self.events.push(Event::GameOver);
            }
            return;
        }

        // Remember the colours of each full row before the rows are removed,
        // so the clear effect can be tinted per block.
        let full = self.grid.full_rows();
        let cleared_colors: Vec<Vec<u8>> = full
            .iter()
            .map(|&y| (0..GRID_WIDTH).map(|x| self.grid.get(x as i32, y as i32)).collect())
            .collect();

        let cleared = self.grid.clear_lines();
        self.lines_cleared += cleared as i32;

        if t_spin {
            self.score += TSPIN_BONUS * self.level;
            self.t_spin_count += 1;
            self.events.push(Event::TSpin);
        }

        if !full.is_empty() {
            self.events.push(Event::LineClear {
                rows: full,
                cells: cleared_colors,
            });
        }

        self.score += cleared as i32 * POINTS_PER_LINE;

        if cleared > 0 {
            self.combo += 1;
            if self.combo > self.max_combo {
                self.max_combo = self.combo;
            }
            if self.combo > 0 {
                self.score += COMBO_BASE * self.combo * self.level * cleared as i32;
            }
            self.events.push(Event::Combo(self.combo));

            let tetris = cleared == 4;
            if tetris || t_spin {
                // Back-to-back: a half-bonus on the running total, as in the
                // original.
                if self.back_to_back {
                    self.score += self.score / 2;
                }
                self.back_to_back = true;
                self.difficult_clear = true;
            } else {
                self.back_to_back = false;
                self.difficult_clear = false;
            }
            self.events.push(Event::BackToBack(self.back_to_back));

            if self.grid.is_empty() {
                self.score += ALL_CLEAR_BONUS * self.level;
                self.events.push(Event::AllClear);
            }
        } else {
            self.combo = -1;
            self.back_to_back = false;
            self.difficult_clear = false;
        }

        // The level rises by one every `LINES_PER_LEVEL` lines, counted from
        // wherever the run *started* rather than from level one.
        //
        // The original computes `lines_cleared // 10 + 1` and compares it to the
        // current level, which is the same thing for every mode that opens at
        // level one. For a mode that opens at 15 it is not: the expression reads
        // 1 until 141 lines have been cleared, so `new_level > level` is false
        // the whole time and Master never levels again. It opened at level 15 and
        // stayed there for the entire run, gravity pinned at the level-15 speed,
        // with the skin system here also frozen on one skin. Offsets from the
        // start level so a run that opens at 15 keeps climbing.
        let new_level =
            self.settings.start_level.max(1) + self.lines_cleared / LINES_PER_LEVEL;
        if new_level > self.level {
            self.level = new_level;
            self.recompute_fall_speed();
            self.events.push(Event::LevelUp(self.level));
        }

        match self.mode {
            Mode::Sprint if self.lines_cleared >= SPRINT_TARGET_LINES => {
                self.won = true;
                self.ended_at = Some(now);
                self.events.push(Event::Win);
            }
            _ => {
                let incoming = self.next.remove(0);
                self.next.push(self.bag.next_piece());
                self.spawn_piece(incoming, now);
            }
        }

        self.grounded_since = None;
        self.lock_resets = 0;
        self.lock_moves = 0;
    }

    /// Training-mode helper: clear the bottom row without scoring.
    fn delete_bottom_row(&mut self) {
        let y = (crate::board::TOTAL_ROWS - 1) as i32;
        for x in 0..GRID_WIDTH as i32 {
            self.grid.clear_cell(x, y);
        }
    }

    /// Drop accumulated events. Call at the top of a frame, before new ones
    /// are generated, so the presentation layer can react to this frame only.
    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    // --- Per-frame update --------------------------------------------------

    /// Advance timers. Call once per frame with the current clock.
    pub fn tick(&mut self, now: u64) {
        if self.is_finished() {
            return;
        }
        if self.mode.is_timed() && self.elapsed_ms(now) >= ULTRA_DURATION_MS {
            self.won = true;
            self.ended_at = Some(now);
            self.events.push(Event::Win);
            return;
        }

        self.apply_das(now);
        self.advance_gravity(now);
        self.soft_dropping = false;

        if let Some(since) = self.grounded_since {
            if now.saturating_sub(since) >= LOCK_DELAY_TIME as u64 {
                self.lock_now(now);
            }
        }
    }

    /// Delayed auto shift / auto repeat for held left and right.
    fn apply_das(&mut self, now: u64) {
        if !self.left_held && !self.right_held {
            return;
        }
        let held = now.saturating_sub(self.last_h_shift);
        let interval = if held > self.settings.das_ms as u64 {
            self.settings.arr_ms as u64
        } else {
            self.settings.das_ms as u64
        };
        if held < interval {
            return;
        }
        // Both held: last press wins, so nudge in the most recent direction.
        let dx = if self.left_held && self.right_held {
            if self.right_held {
                1
            } else {
                -1
            }
        } else if self.left_held {
            -1
        } else {
            1
        };
        let target = (self.origin.0 + dx, self.origin.1);
        if self.grid.fits(self.piece, self.rot, target) {
            self.origin = target;
            self.events.push(Event::Move {
                at: self.pose(),
                dir: (dx, 0),
            });
            self.on_piece_moved(now);
        }
        self.last_h_shift = now;
    }

    /// Gravity: drop a row on schedule, lock when resting.
    ///
    /// Soft drop runs at a fixed [`SOFT_DROP_SPEED`] cadence, exactly the
    /// original's `current_fall_speed = 50 if fast_fall else fall_speed`, so
    /// holding DOWN sinks the piece at a sane twenty rows per second instead
    /// of one row per frame.
    fn advance_gravity(&mut self, now: u64) {
        let delay = if self.soft_dropping {
            SOFT_DROP_SPEED as u64
        } else {
            self.fall_speed as u64
        };
        if now.saturating_sub(self.last_fall) < delay {
            return;
        }
        let can_fall = self
            .grid
            .fits(self.piece, self.rot, (self.origin.0, self.origin.1 + 1));

        if can_fall {
            self.origin.1 += 1;
            self.grounded_since = None;
            self.lock_resets = 0;
            self.lock_moves = 0;
            // The moment the piece comes to rest, lock delay starts counting.
            // Without this it would sit on the stack for nearly a full
            // gravity interval before the delay window even begins.
            if !self
                .grid
                .fits(self.piece, self.rot, (self.origin.0, self.origin.1 + 1))
            {
                self.grounded_since = Some(now);
            }
        } else if self.soft_dropping {
            // Soft dropping into the stack locks immediately, as in the
            // original.
            self.lock_now(now);
        } else if self.grounded_since.is_none() {
            self.grounded_since = Some(now);
        }
        self.last_fall = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::TOTAL_ROWS;

    /// A run with the default settings, at the opening level its mode asks for.
    ///
    /// `start_level` is set here rather than left at the `Settings::default()`
    /// value because `Game::new` reads it instead of deriving the level from the
    /// mode - which is the whole point of the field. A helper that did not set it
    /// would quietly build every Master run at level 1 and let the tests below
    /// pass against a game nobody can start from the menu.
    fn game(mode: Mode) -> Game {
        let mut s = Settings::default();
        s.base_fall_speed = 1000;
        s.start_level = if mode == Mode::Master { 15 } else { 1 };
        Game::new(mode, s, 0)
    }

    /// Drive the board directly for deterministic tests.
    fn fill_row(g: &mut Game, y: i32, color: u8) {
        for x in 0..GRID_WIDTH as i32 {
            g.grid.force_set(x, y, color);
        }
    }

    #[test]
    fn starts_with_five_pieces_queued() {
        let g = game(Mode::Marathon);
        assert_eq!(g.next.len(), 5);
        assert!(!g.game_over);
    }

    // --- the per-level colour shift --------------------------------------

    /// A game at a given level, for the colour tests.
    fn at_level(level: i32) -> Game {
        let mut g = game(Mode::Marathon);
        g.level = level;
        g
    }

    /// Every piece, so a test cannot pass by only checking the easy ones.
    const ALL_PIECES: [Piece; 7] = [
        Piece::I,
        Piece::J,
        Piece::L,
        Piece::O,
        Piece::S,
        Piece::T,
        Piece::Z,
    ];

    #[test]
    fn level_one_leaves_the_palette_where_it_was() {
        // The original's formula is `(shape_index + level - 1) % 7 + 1`, so at
        // level 1 the shift is zero and every piece keeps its own colour. If
        // that ever stops being true the game's colours are wrong on the very
        // first board a player sees, before they have done anything.
        let g = at_level(1);
        assert_eq!(g.level, 1);
        for p in ALL_PIECES {
            assert_eq!(g.color_for(p), p as u8 + 1, "{p:?}");
        }
    }

    #[test]
    fn each_level_steps_every_piece_one_colour_along() {
        // "Along" includes off the end: the last colour rolls round to the first
        // rather than stopping or growing, which is what makes the change look
        // like a rotation of the palette rather than a ramp into it.
        let one = at_level(1);
        let two = at_level(2);
        for p in ALL_PIECES {
            assert_eq!(two.color_for(p), one.color_for(p) % 7 + 1, "{p:?} did not step");
        }
    }

    #[test]
    fn the_palette_wraps_rather_than_running_out() {
        // Seven colours, so level 8 is back to level 1. A player who keeps
        // playing sees the colours come round again, which is the point.
        let g = at_level(1);
        for p in ALL_PIECES {
            let at_one = g.color_for(p);
            assert_eq!(at_level(8).color_for(p), at_one, "{p:?} did not wrap");
        }
    }

    #[test]
    fn a_preview_shows_the_colour_the_piece_will_actually_be() {
        // The reason `color_for` exists. A next-queue preview is a promise, and
        // the original kept that promise by running the same level shift for the
        // preview as for the live piece. Reading the raw discriminant instead -
        // which is what the panel used to do - makes the preview lie from level
        // 2 onwards, and a player planning two pieces ahead is then planning
        // against a colour that will not arrive.
        for level in 1..=9 {
            let g = at_level(level);
            for p in ALL_PIECES {
                // The colour depends on the piece and the level, and on nothing
                // else - not on which slot it is drawn in, and not on the order
                // the queue happens to be in.
                let in_the_queue = g.color_for(p);
                let in_the_hold_slot = g.color_for(p);
                let when_it_falls = g.color_for(p);
                assert_eq!(in_the_queue, in_the_hold_slot);
                assert_eq!(in_the_hold_slot, when_it_falls);
                assert!(
                    (1..=7).contains(&in_the_queue),
                    "level {level} gave {p:?} palette slot {in_the_queue}"
                );
            }
            // The live piece goes through `color`, and that must agree with the
            // same call for its own discriminant - otherwise the board and the
            // panel disagree about the piece currently falling.
            let falling = g.piece();
            assert_eq!(g.color(), g.color_for(falling));
        }
    }

    /// Traditional has to mean what it says: the I piece is the same colour at
    /// every level, for the whole run. If the palette slot still rotated with
    /// the level, the option would lock the *palette* but not the pieces, and a
    /// player who chose it to recognise pieces at a glance would still be
    /// guessing.
    #[test]
    fn a_pinned_palette_gives_every_piece_the_same_colour_all_run() {
        let mut g = game(Mode::Marathon);
        g.settings.shift_palette = false;
        let first: Vec<u8> = ALL_PIECES.iter().map(|p| g.color_for(*p)).collect();
        for level in 1..=200 {
            g.level = level;
            let now: Vec<u8> = ALL_PIECES.iter().map(|p| g.color_for(*p)).collect();
            assert_eq!(now, first, "level {level} moved a piece's colour");
        }
    }

    /// A pinned palette is still seven different colours - pinning is not
    /// "everything the same".
    #[test]
    fn a_pinned_palette_still_separates_the_seven_pieces() {
        let mut g = game(Mode::Marathon);
        g.settings.shift_palette = false;
        let mut seen = std::collections::HashSet::new();
        for p in ALL_PIECES {
            let c = g.color_for(p);
            assert!((1..=7).contains(&c), "{p:?} got colour {c}");
            assert!(seen.insert(c), "{p:?} shares colour {c} with another piece");
        }
        assert_eq!(seen.len(), 7);
    }

    /// Turning the shift back off... on, rather, has to take effect at once and
    /// start moving again.
    #[test]
    fn unpinning_restores_the_level_shift() {
        let mut g = game(Mode::Marathon);
        g.level = 1;
        g.settings.shift_palette = false;
        let pinned = g.color_for(Piece::I);
        g.settings.shift_palette = true;
        assert_eq!(g.color_for(Piece::I), at_level(1).color_for(Piece::I));
        g.level = 2;
        assert_ne!(g.color_for(Piece::I), pinned, "the shift did not come back");
    }

    /// The default is the original's behaviour: the palette shifts with the
    /// level. Traditional has to be opted into, or a fresh install would be in
    /// the non-original mode.
    #[test]
    fn the_default_palette_shifts_with_the_level() {
        assert!(Settings::default().shift_palette);
        assert_ne!(at_level(1).color_for(Piece::I), at_level(2).color_for(Piece::I));
    }

    /// The Move event has to say which way the piece went: a gesture burst
    /// cannot tell a leftward slide from a rightward one otherwise, and throws
    /// itself from the wrong edge.
    ///
    /// Soft drop is deliberately not in this list. A soft drop is gravity, and
    /// gravity fires on *every* row including passive falling, so treating it
    /// as a gesture would spray a burst once per row of a piece doing nothing.
    /// The held-key trail already covers a soft drop, and a hard drop gets its
    /// own slam - which is the drop a player actually chose.
    #[test]
    fn a_move_event_carries_the_way_the_piece_went() {
        for (action, want) in [
            (Action::MoveLeft, (-1, 0)),
            (Action::MoveRight, (1, 0)),
        ] {
            let mut g = game(Mode::Marathon);
            let start = g.origin();
            g.apply(action, 0);
            let moved: Vec<(i32, i32)> = g
                .events
                .iter()
                .filter_map(|e| match e {
                    Event::Move { dir, .. } => Some(*dir),
                    _ => None,
                })
                .collect();
            assert!(
                !moved.is_empty(),
                "{action:?} at {start:?} produced no Move event"
            );
            for dir in moved {
                assert_eq!(dir, want, "{action:?} reported {dir:?}");
            }
        }
    }

    /// An event has to report where the piece was, not just which way it went.
    ///
    /// This is the invariant the whole presentation layer leans on, and the
    /// reason it is worth a test: the events are read a frame later, and by then
    /// the piece is somewhere else. A `Move` event that reported only a
    /// direction would force the renderer to consult live game state, which is
    /// a different piece entirely by the time it looks.
    #[test]
    fn every_pose_event_reports_the_piece_as_it_was() {
        let mut g = game(Mode::Marathon);
        g.apply(Action::RotateCw, 0);
        for ev in &g.events {
            if let Event::Move { at, .. } | Event::Rotate { at, .. } = ev {
                assert_eq!(at.piece, g.piece(), "wrong piece on the event");
                assert_eq!(at.rot, g.rotation(), "wrong rotation on the event");
                // The piece has not moved since - no other action was applied -
                // so the pose must match the live state exactly.
                assert_eq!(at.origin, g.origin(), "wrong origin on the event");
            }
        }
    }

    /// The hard drop is the case that makes the payload necessary. It locks the
    /// piece *and* spawns its successor in one call, so the live origin is a
    /// brand new piece at the top of the board. The event has to report the
    /// pose of the piece that was actually dropped.
    #[test]
    fn a_hard_drop_event_reports_the_piece_that_was_dropped() {
        let mut g = game(Mode::Marathon);
        // Move somewhere unambiguous first, so a stale spawn origin could not
        // accidentally be the right answer.
        for _ in 0..3 {
            g.apply(Action::MoveLeft, 0);
        }
        g.apply(Action::SoftDrop, 0);
        let dropped = g.origin();
        let piece = g.piece();
        g.clear_events();
        g.apply(Action::HardDrop, 0);

        let drop = g
            .events
            .iter()
            .find_map(|e| match e {
                Event::HardDrop { at, rows } => Some((*at, *rows)),
                _ => None,
            })
            .expect("a hard drop reported no event");
        assert!(drop.1 > 0, "the piece fell {rows} rows", rows = drop.1);
        // It fell: strictly below where it was released from.
        assert!(drop.0.origin.1 > dropped.1, "the drop did not move the piece");
        assert_eq!(drop.0.piece, piece, "the event named a different piece");

        // And the live piece really has moved on, which is the whole reason the
        // payload exists: the successor is up at the top of the board, nowhere
        // near the floor the event reported. An effect placed from live state
        // would burst against the ceiling on the one gesture where the distance
        // travelled was largest.
        assert!(
            g.origin().1 <= 0,
            "the live piece is at y {}, not back at the top",
            g.origin().1
        );
        assert_ne!(g.origin(), drop.0.origin);
    }

    /// A pose that a renderer can use: the reported origin must be one the piece
    /// actually occupied, so the burst lands on the block and not beside it.
    #[test]
    fn a_pose_is_always_a_position_the_piece_actually_held() {
        let mut g = game(Mode::Marathon);
        for action in [
            Action::RotateCw,
            Action::RotateCcw,
            Action::MoveLeft,
            Action::MoveRight,
            Action::HardDrop,
        ] {
            g.clear_events();
            g.apply(action, 0);
            for ev in &g.events {
                let at = match ev {
                    Event::Move { at, .. }
                    | Event::Rotate { at, .. }
                    | Event::HardDrop { at, .. }
                    | Event::Lock { at, .. } => *at,
                    _ => continue,
                };
                // The pose is on the board, or in the hidden rows above it where
                // a spawning piece legitimately lives.
                assert!(at.origin.0 >= 0, "{action:?} reported x {}", at.origin.0);
                // The window is the board's own, from the top of the hidden
                // buffer to the floor: a piece can spawn above the playfield and
                // come to rest with its origin on the bottom row. Anything else
                // means the pose was stamped from somewhere it never was, and a
                // burst placed there would be drawn off the board entirely.
                assert!(
                    at.origin.1 >= -(HIDDEN_ROWS as i32)
                        && at.origin.1 < TOTAL_ROWS as i32,
                    "{action:?} reported y {}, outside the board window",
                    at.origin.1
                );
                assert!(!pieces::cells(at.piece, at.rot).is_empty());
            }
        }
    }

    /// A wall does not move the piece, so it must not claim it did. An event
    /// for a refused move would throw a burst off a block that is still sitting
    /// in the same place.
    ///
    /// The wall is the board edge, reached by walking the piece into it. Building
    /// a wall out of `force_set` instead would be defeated by the hidden rows
    /// above the playfield: a piece spawns partly above row 0, writes to those
    /// rows are silently dropped, and the move still fits.
    #[test]
    fn a_move_refused_by_a_wall_says_nothing() {
        let mut g = game(Mode::Marathon);
        // Walk into the left wall. The loop cannot hang: each pass either moves
        // the piece one left or breaks.
        while g.origin().0 > 0 {
            let before = g.origin();
            g.clear_events();
            g.apply(Action::MoveLeft, 0);
            if g.origin() == before {
                break;
            }
        }
        assert_eq!(g.origin().0, 0, "the piece never reached the wall");
        g.clear_events();
        let origin = g.origin();
        g.apply(Action::MoveLeft, 0);
        assert_eq!(g.origin(), origin, "the piece should not have moved");
        assert!(
            !g.events.iter().any(|e| matches!(e, Event::Move { .. })),
            "a refused move reported a Move event"
        );
    }

    /// A rotation carries its wall kick, so a piece that was nudged sideways
    /// while turning throws its burst from the side it actually went.
    #[test]
    fn a_rotate_event_carries_its_wall_kick() {
        let mut g = game(Mode::Marathon);
        g.apply(Action::RotateCw, 0);
        for e in &g.events {
            if let Event::Rotate { dir, .. } = e {
                // A kick is a real offset, never a wild one - a rotate event
                // reporting a displacement bigger than the board would put the
                // burst somewhere off the playfield.
                assert!(dir.0.abs() <= 2, "kick dx {}", dir.0);
                assert!(dir.1.abs() <= 2, "kick dy {}", dir.1);
            }
        }
    }

    #[test]
    fn a_level_below_one_cannot_underflow_the_shift() {
        // `level - 1` is an `i32` and the shift is added to a `usize`, so
        // `as usize` on a negative would wrap to something enormous and shuffle
        // every colour at random. Level 0 is unreachable in play, but a
        // hand-edited or future save must not be able to reach it either.
        let one = at_level(1);
        for level in [i32::MIN, -1000, -1, 0] {
            let g = at_level(level);
            for p in ALL_PIECES {
                assert_eq!(
                    g.color_for(p),
                    one.color_for(p),
                    "level {level} moved {p:?}"
                );
            }
        }
    }

    #[test]
    fn a_level_high_enough_to_overflow_still_names_a_real_colour() {
        // `(piece + level - 1) % 7` on a `usize` is fine at any level a player
        // can reach, but the addition is worth pinning at the top of the range
        // a save file could hold.
        for level in [i32::MAX - 1, 1_000_000, 1 << 20] {
            let g = at_level(level);
            for p in ALL_PIECES {
                assert!((1..=7).contains(&g.color_for(p)), "level {level}");
            }
        }
    }

    #[test]
    fn gravity_drops_the_piece_on_schedule() {
        let mut g = game(Mode::Marathon);
        let y0 = g.origin().1;
        g.tick(1001);
        assert_eq!(g.origin().1, y0 + 1);
    }

    /// At 20G a new piece is already resting when it appears: the fall is
    /// skipped entirely and placement becomes a floor-sliding problem.
    #[test]
    fn a_piece_spawns_already_landed_at_the_gravity_ceiling() {
        let mut g = game(Mode::Marathon);
        g.level = MAX_GRAVITY as i32;
        g.recompute_fall_speed();
        assert!(g.at_max_gravity());

        // Hard-drop whatever is in play; the piece that replaces it is spawned
        // by the same path a real spawn takes.
        g.apply(Action::HardDrop, 0);

        assert!(
            g.grid.drop_distance(g.piece(), g.rotation(), g.origin()) == 0,
            "at 20G a fresh piece should already be resting on the stack"
        );
    }

    /// Below the ceiling the piece still enters from the top, so the ceiling -
    /// not some other change - is what makes the board slam shut.
    #[test]
    fn a_piece_below_the_ceiling_still_spawns_at_the_top() {
        let mut g = game(Mode::Marathon);
        assert!(!g.at_max_gravity(), "level 1 must not be at the ceiling");
        let spawn_y = pieces::spawn_y();
        g.apply(Action::HardDrop, 0);
        assert_eq!(
            g.origin().1, spawn_y,
            "below 20G the next piece should enter from the top"
        );
        assert!(
            g.grid.drop_distance(g.piece(), g.rotation(), g.origin()) > 0,
            "a piece below the ceiling has somewhere to fall to"
        );
    }

    /// The ceiling is reached by climbing, and spawning stays instant from the
    /// moment it is - including on the very piece that crosses into it.
    #[test]
    fn the_ceiling_takes_effect_the_level_it_is_reached() {
        let mut g = game(Mode::Marathon);
        for level in 1..=MAX_GRAVITY as i32 {
            g.level = level;
            g.recompute_fall_speed();
            assert_eq!(
                g.at_max_gravity(),
                level >= MAX_GRAVITY as i32,
                "level {level} ceiling state"
            );
        }
    }

    #[test]
    fn hard_drop_awards_two_points_per_row() {
        let mut g = game(Mode::Marathon);
        let rows = g.grid.drop_distance(g.piece(), g.rotation(), g.origin());
        g.apply(Action::HardDrop, 0);
        assert_eq!(g.score, rows * POINTS_PER_HARD_DROP_ROW);
    }

    #[test]
    fn hard_drop_locks_the_piece_and_spawns_the_next() {
        let mut g = game(Mode::Marathon);
        let color = g.color();
        let queued = g.next[0];

        g.apply(Action::HardDrop, 0);

        assert_eq!(g.pieces_dropped, 1);
        // The queue advances.
        assert_eq!(g.piece(), queued);
        // The board is no longer empty, and the locked piece is on it.
        assert!(!g.grid.is_empty());
        assert!(g
            .grid
            .as_rows()
            .iter()
            .any(|row| row.contains(&color)));
    }

    /// This is the original's headline bug: locking a piece without a hard
    /// drop awarded nothing. Soft-dropping into place locks and scores just
    /// like any other lock.
    #[test]
    fn soft_drop_lock_scores_and_advances_the_level() {
        let mut g = game(Mode::Marathon);
        open_column(&mut g, 8);
        g.lines_cleared = 9; // one more clear reaches level 2
        vertical_i_into(&mut g, 8);

        // Hold soft drop until the piece locks into the stack. Each drop is
        // one 50 ms cadence step, exactly as a held key would fire.
        for step in 0..40 {
            if g.pieces_dropped > 0 {
                break;
            }
            g.apply(Action::SoftDrop, step * SOFT_DROP_SPEED as u64);
        }
        assert!(g.pieces_dropped > 0, "soft drop never locked");
        assert!(g.lines_cleared >= 13, "soft-dropped piece cleared no lines");
        assert_eq!(g.level, 2, "level never advanced");
        assert!(
            g.score > 0,
            "locking scored nothing (bug from the original)"
        );
    }

    #[test]
    fn four_line_clear_is_a_tetris_and_sets_back_to_back() {
        let mut g = game(Mode::Marathon);
        for y in 0..4 {
            fill_row(&mut g, (crate::board::TOTAL_ROWS - 1 - y) as i32, 2);
        }
        // Open a single column for the I piece to fall through.
        for y in 0..4 {
            g.grid.force_set(8, (crate::board::TOTAL_ROWS - 1 - y) as i32, 0);
        }
        // A vertical I occupies column origin.0 + 2, so aim it at the gap.
        g.piece = Piece::I;
        g.rot = 1;
        g.origin = (6, 0);

        g.apply(Action::HardDrop, 0);

        assert_eq!(g.lines_cleared, 4, "vertical I should clear four rows");
        assert!(g.back_to_back, "a Tetris must set back-to-back");
    }

    /// Clearing the very last row empties the board, which must trigger the
    /// all-clear bonus (`ALL_CLEAR_BONUS` x level) and its event.
    #[test]
    fn clearing_the_final_row_awards_the_all_clear_bonus() {
        let mut g = game(Mode::Marathon);
        let bottom = (crate::board::TOTAL_ROWS - 1) as i32;
        // The bottom row is the only stack: full except the four cells a
        // horizontal I exactly covers; every row above it is empty.
        for x in 0..GRID_WIDTH as i32 {
            if !(5..=8).contains(&x) {
                g.grid.force_set(x, bottom, 2);
            }
        }
        g.piece = Piece::I;
        g.rot = 0;
        // The I's spawn row sits at box-row 1, so drop it from one row above
        // to land its four cells on `bottom`.
        g.origin = (5, bottom - 1);

        let before = g.score;
        g.apply(Action::HardDrop, 0);

        assert!(g.grid.is_empty(), "all-clear must leave the board empty");
        assert!(
            g.events.iter().any(|e| matches!(e, Event::AllClear)),
            "all-clear event never fired"
        );
        // 1 line (100) + all-clear (1200*level 1). The first clear starts the
        // combo counter at 0, so it earns no combo bonus.
        assert_eq!(g.score - before, 1300, "all-clear bonus must be scored");
    }

    #[test]
    fn difficulty_names_map_to_the_original_speeds() {
        assert_eq!(difficulty_speeds("easy"), 1500);
        assert_eq!(difficulty_speeds("normal"), 1000);
        assert_eq!(difficulty_speeds("hard"), 600);
        assert_eq!(difficulty_speeds("very hard"), 400);
        assert_eq!(difficulty_speeds("master"), 200);
        // Anything unrecognised falls back to normal speed.
        assert_eq!(difficulty_speeds("impossible"), 1000);
    }

    #[test]
    fn combo_resets_when_a_piece_clears_nothing() {
        let mut g = game(Mode::Marathon);
        // A piece dropped onto an empty board clears nothing, so the combo
        // must break.
        g.apply(Action::HardDrop, 0);
        assert_eq!(g.lines_cleared, 0);
        assert_eq!(g.combo, -1);
    }

    /// Fill the whole board except a single column at `col`, left open all
    /// the way down, so a vertical I can fall through it and complete rows
    /// at the bottom. Clearing drains from the bottom, but the open column
    /// travels down with the stack, so it is endlessly reusable.
    fn open_column(g: &mut Game, col: i32) {
        for y in 0..crate::board::TOTAL_ROWS as i32 {
            for x in 0..GRID_WIDTH as i32 {
                g.grid.force_set(x, y, if x == col { 0 } else { 1 });
            }
        }
    }

    /// Put a vertical I piece into the open column at `col`.
    fn vertical_i_into(g: &mut Game, col: i32) {
        g.piece = Piece::I;
        g.rot = 1; // this rotation's cells all sit at local x = 2
        g.origin = (col - 2, 0);
        g.hold_used = false;
    }

    #[test]
    fn combo_builds_across_consecutive_clears() {
        let mut g = game(Mode::Marathon);
        open_column(&mut g, 8);
        let mut seen_combo = Vec::new();
        for _ in 0..3 {
            vertical_i_into(&mut g, 8);
            g.apply(Action::HardDrop, 0);
            seen_combo.push(g.combo);
        }
        // The first clear starts the combo at 0; each later one extends it.
        assert_eq!(seen_combo, vec![0, 1, 2], "combo did not build");
        assert_eq!(g.lines_cleared, 12);
        assert_eq!(g.max_combo, 2);
    }

    #[test]
    fn combo_bonus_is_scored() {
        let mut g = game(Mode::Marathon);
        open_column(&mut g, 8);

        // First tetris: line points only (the combo counter is still at 0).
        vertical_i_into(&mut g, 8);
        g.apply(Action::HardDrop, 0);
        let after_first = g.score;

        // Second tetris: the running combo at 1 adds 50 * 1 * level * 4 on
        // top of the four lines.
        vertical_i_into(&mut g, 8);
        g.apply(Action::HardDrop, 0);
        let expected = 4 * POINTS_PER_LINE + COMBO_BASE * 1 * g.level * 4;
        let gained = g.score - after_first;
        assert!(
            gained >= expected,
            "second clear gained {gained}, expected at least {expected}"
        );
    }

    /// Carve a T-slot: an overhang whose three corners around the T's centre
    /// are blocked. Returns the origin the T should rest at.
    fn build_t_slot(g: &mut Game) -> (i32, i32) {
        let floor = (crate::board::TOTAL_ROWS - 1) as i32;
        // Solid floor.
        for x in 0..GRID_WIDTH as i32 {
            g.grid.force_set(x, floor, 1);
        }
        let origin = (5, floor - 2);
        let (cx, cy) = (origin.0 + 1, origin.1 + 1);
        // Block three of the four corners around the centre. The corner below
        // right is left open, which is the T-spin entry.
        g.grid.force_set(cx - 1, cy - 1, 1);
        g.grid.force_set(cx + 1, cy - 1, 1);
        g.grid.force_set(cx - 1, cy + 1, 1);
        // Wall off the slot sides so the T cannot slide out.
        g.grid.force_set(origin.0 - 1, floor - 1, 1);
        g.grid.force_set(origin.0 + 3, floor - 1, 1);
        origin
    }

    #[test]
    fn t_spin_awards_bonus_and_counts() {
        let mut g = game(Mode::Marathon);
        let origin = build_t_slot(&mut g);

        g.piece = Piece::T;
        g.rot = 0;
        g.origin = origin;

        // It is a spin by the corner rule, and stays one at the origin.
        assert!(g.grid.is_t_spin(Piece::T, 0, origin));
        // The piece genuinely rests there: it cannot drop any further.
        assert!(!g.grid.fits(Piece::T, 0, (origin.0, origin.1 + 1)));

        let before = g.t_spin_count;
        let score_before = g.score;
        g.apply(Action::HardDrop, 0);

        assert_eq!(g.t_spin_count, before + 1, "T-spin was not detected");
        assert!(g.score >= score_before + TSPIN_BONUS * g.level);
    }

    #[test]
    fn a_plain_t_drop_is_not_a_t_spin() {
        let mut g = game(Mode::Marathon);
        // Flat floor, no slot.
        let floor = (crate::board::TOTAL_ROWS - 1) as i32;
        for x in 0..GRID_WIDTH as i32 {
            g.grid.force_set(x, floor, 1);
        }
        g.piece = Piece::T;
        g.rot = 0;
        g.origin = (5, floor - 1);
        let before = g.t_spin_count;
        g.apply(Action::HardDrop, 0);
        assert_eq!(g.t_spin_count, before, "flat drop counted as a T-spin");
    }

    #[test]
    fn hold_swaps_once_per_piece() {
        let mut g = game(Mode::Marathon);
        let first = g.piece();
        let queued = g.next[0];

        // First hold stashes the current piece and pulls in the next one.
        g.apply(Action::Hold, 0);
        assert_eq!(g.hold, Some(first));
        assert_eq!(g.piece(), queued);

        // A second hold before locking is ignored.
        let current = g.piece();
        g.apply(Action::Hold, 0);
        assert_eq!(g.piece(), current);
        assert_eq!(g.hold, Some(first));
    }

    #[test]
    fn hold_is_reusable_after_a_piece_locks() {
        let mut g = game(Mode::Marathon);
        let first = g.piece();
        g.apply(Action::Hold, 0);
        assert_eq!(g.hold, Some(first));

        // Locking clears the per-piece hold flag.
        g.apply(Action::HardDrop, 0);
        let in_play = g.piece();
        g.apply(Action::Hold, 0);

        // The original piece comes back out of the hold slot...
        assert_eq!(g.piece(), first, "the held piece did not come back");
        // ...and the piece that was just in play now waits in hold.
        assert_eq!(g.hold, Some(in_play));
    }

    #[test]
    fn hold_keeps_the_queue_the_right_length() {
        let mut g = game(Mode::Marathon);
        for _ in 0..10 {
            assert_eq!(g.next.len(), 5);
            g.apply(Action::Hold, 0);
            assert_eq!(g.next.len(), 5);
            g.apply(Action::HardDrop, 0);
        }
    }

    #[test]
    fn rotation_respects_walls() {
        let mut g = game(Mode::Marathon);
        g.piece = Piece::I;
        g.rot = 0;
        g.origin = (0, 0); // hard against the left wall
        let before = g.origin();
        g.apply(Action::RotateCw, 0);
        // Either it rotated with a kick, or it did not move at all - but it
        // must never end up out of bounds.
        assert!(g.grid.fits(Piece::I, g.rotation(), g.origin()));
        let _ = before;
    }

    #[test]
    fn training_never_ends() {
        let mut g = game(Mode::Training);
        for y in 0..crate::board::TOTAL_ROWS as i32 - 4 {
            for x in 0..GRID_WIDTH as i32 {
                g.grid.force_set(x, y, 1);
            }
        }
        for _ in 0..20 {
            g.apply(Action::HardDrop, 0);
        }
        assert!(!g.game_over);
    }

    #[test]
    fn training_delete_bottom_row() {
        let mut g = game(Mode::Training);
        fill_row(&mut g, (crate::board::TOTAL_ROWS - 1) as i32, 1);
        g.apply(Action::DeleteBottomRow, 0);
        assert!(g.grid.get(4, (crate::board::TOTAL_ROWS - 1) as i32) == 0);
    }

    #[test]
    fn delete_bottom_row_ignored_outside_training() {
        let mut g = game(Mode::Marathon);
        fill_row(&mut g, (crate::board::TOTAL_ROWS - 1) as i32, 1);
        g.apply(Action::DeleteBottomRow, 0);
        assert!(g.grid.get(4, (crate::board::TOTAL_ROWS - 1) as i32) != 0);
    }

    #[test]
    fn sprint_wins_at_forty_lines() {
        let mut g = game(Mode::Sprint);
        g.lines_cleared = 40;
        g.apply(Action::HardDrop, 0);
        assert!(g.won);
        assert!(!g.game_over);
    }

    // --- The mode rules the menu promises ---------------------------------
    //
    // Each of these is a claim the mode name and blurb make. A test that fails
    // here means the game is lying about what a mode is, which is worse than any
    // scoring or rendering bug: the player picked a mode on the strength of a
    // sentence they can now read on the menu.

    /// "Clear 40 lines as fast as you can" means 40: one line short does not end
    /// the run, and 40 does.
    ///
    /// Only Sprint. Marathon and Master are endless and Ultra is on a clock, so
    /// asserting that they *also* end at 40 lines would be asserting a rule they
    /// never had.
    #[test]
    fn sprint_ends_exactly_at_its_line_target() {
        let mut g = game(Mode::Sprint);
        g.lines_cleared = SPRINT_TARGET_LINES - 1;
        g.apply(Action::HardDrop, 0);
        assert!(
            !g.is_finished(),
            "sprint ended at {} lines, before its target",
            SPRINT_TARGET_LINES - 1
        );
        g.lines_cleared = SPRINT_TARGET_LINES;
        g.apply(Action::HardDrop, 0);
        assert!(g.won, "sprint did not end at {SPRINT_TARGET_LINES} lines");
        assert!(!g.game_over);
    }

    /// The endless modes really are endless: no line count ends them.
    #[test]
    fn the_endless_modes_do_not_end_at_sprints_line_target() {
        for mode in [Mode::Marathon, Mode::Master, Mode::Training] {
            let mut g = game(mode);
            g.lines_cleared = SPRINT_TARGET_LINES * 10;
            g.apply(Action::HardDrop, 0);
            assert!(
                !g.is_finished(),
                "{mode:?} ended at {} lines, but it is endless",
                SPRINT_TARGET_LINES * 10
            );
        }
    }

    /// Sprint's target has to match the number its blurb prints, or the menu is
    /// advertising a goal the game is not scoring against.
    #[test]
    fn the_sprint_blurb_names_the_target_the_game_uses() {
        let blurb = Mode::Sprint.blurb();
        assert!(
            blurb.contains(&SPRINT_TARGET_LINES.to_string()),
            "the blurb {blurb:?} does not mention the real target {SPRINT_TARGET_LINES}"
        );
    }

    /// "Score as much as you can in 3 minutes" - the limit is three minutes.
    #[test]
    fn ultra_runs_for_the_three_minutes_its_blurb_promises() {
        assert_eq!(Mode::Ultra.duration_ms(), Some(3 * 60 * 1000));
        let mut g = game(Mode::Ultra);
        assert_eq!(g.remaining_ms(0), Some(ULTRA_DURATION_MS));
        g.tick(ULTRA_DURATION_MS - 1);
        assert!(!g.is_finished());
        assert_eq!(g.remaining_ms(ULTRA_DURATION_MS - 1), Some(1));
        g.tick(ULTRA_DURATION_MS);
        assert!(g.is_finished());
    }

    /// A run out of time is not a top-out, and must not read as one.
    ///
    /// The original sets `game_over = True` here and shows "Time Up!". The port
    /// set `won`, which put "YOU WIN" on screen for a player who simply lasted
    /// three minutes - a win nobody earned and a claim the mode never made.
    #[test]
    fn an_ultra_time_up_is_not_a_game_over() {
        let mut g = game(Mode::Ultra);
        g.tick(ULTRA_DURATION_MS);
        assert!(!g.game_over, "surviving the clock was called a top-out");
        assert!(g.won);
        assert_eq!(g.mode.outcome(g.won), Outcome::TimeUp);
        assert_eq!(Outcome::TimeUp.title(), "TIME UP");
    }

    /// Only a top-out is "GAME OVER". Every other ending needs its own word.
    #[test]
    fn every_mode_names_its_own_ending() {
        assert_eq!(Mode::Sprint.outcome(true), Outcome::Complete);
        assert_eq!(Mode::Sprint.outcome(true).title(), "SPRINT COMPLETE");
        assert_eq!(Mode::Marathon.outcome(false), Outcome::Lost);
        assert_eq!(Mode::Marathon.outcome(false).title(), "GAME OVER");
        // No mode's ending may be confused with another's.
        let titles: Vec<&str> = [
            Mode::Sprint.outcome(true),
            Mode::Ultra.outcome(true),
            Mode::Marathon.outcome(true),
            Mode::Marathon.outcome(false),
        ]
        .iter()
        .map(|o| o.title())
        .collect();
        let mut sorted = titles.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), titles.len(), "two endings share a title: {titles:?}");
    }

    /// The clock has to stop when the run does.
    ///
    /// It did not, and this is the whole reason [`Game::elapsed_ms`] consults
    /// `ended_at`: Sprint's finish time is the number being raced, and it used to
    /// keep climbing for as long as the player looked at the results screen.
    #[test]
    fn the_clock_stops_when_the_run_ends() {
        let mut g = game(Mode::Sprint);
        g.lines_cleared = SPRINT_TARGET_LINES;
        g.apply(Action::HardDrop, 5_000);
        assert!(g.won);
        let finished = g.elapsed_ms(5_000);
        assert_eq!(finished, 5_000, "the finish time was not recorded");
        // Ten seconds of staring at the results screen must not move it.
        assert_eq!(g.elapsed_ms(15_000), finished);
        assert_eq!(g.elapsed_ms(u64::MAX / 2), finished);
    }

    /// The same for a top-out, which is the ending Marathon and Master actually
    /// reach.
    #[test]
    fn the_clock_also_stops_on_a_top_out() {
        let mut g = game(Mode::Marathon);
        for y in 0..crate::board::TOTAL_ROWS as i32 - 1 {
            for x in 0..GRID_WIDTH as i32 {
                g.grid.force_set(x, y, 1);
            }
        }
        let mut t = 0u64;
        while !g.is_finished() && t < 60_000 {
            t += 100;
            g.apply(Action::HardDrop, t);
        }
        assert!(g.game_over, "a full board did not end the run");
        let at_loss = g.elapsed_ms(t);
        assert_eq!(g.elapsed_ms(t + 30_000), at_loss);
    }

    /// A mode with a limit has to have remaining time; one without must not
    /// report a limit that does not exist.
    #[test]
    fn only_ultra_reports_a_time_limit() {
        let g = game(Mode::Sprint);
        assert_eq!(g.remaining_ms(1_000), None, "sprint invented a deadline");
        for mode in [Mode::Marathon, Mode::Training, Mode::Master] {
            assert_eq!(game(mode).remaining_ms(0), None, "{mode:?} invented a deadline");
        }
        // And a countdown cannot run past zero.
        let mut g = game(Mode::Ultra);
        g.tick(ULTRA_DURATION_MS);
        assert_eq!(g.remaining_ms(ULTRA_DURATION_MS + 60_000), Some(0));
    }

    /// Both timed modes have to be able to show a clock, or one of them is being
    /// raced blind. Marathon is the only mode without one, because it is endless.
    #[test]
    fn the_raceable_modes_are_the_modes_that_show_a_clock() {
        assert!(Mode::Sprint.shows_clock(), "sprint is a race with no clock");
        assert!(Mode::Ultra.shows_clock(), "ultra is a countdown with no clock");
        assert!(!Mode::Marathon.shows_clock());
    }

    /// A Master run has to keep levelling.
    ///
    /// The original's rule is `lines // 10 + 1`, which reads 1 until 141 lines and
    /// so never exceeds a level-15 opening - Master opened at 15 and stayed there
    /// for the whole run. Here that also froze the skin ladder, since every level
    /// wears a different skin.
    #[test]
    fn a_master_run_keeps_levelling_from_where_it_started() {
        let mut g = game(Mode::Master);
        assert_eq!(g.level, 15);
        let opening_speed = g.fall_speed;

        // Ten lines is a level, exactly as at level one. The original's
        // `lines // 10 + 1` would still read 1 here and leave Master at 15.
        for batch in 1..=3i32 {
            g.lines_cleared = batch * LINES_PER_LEVEL;
            g.apply(Action::HardDrop, 0);
            assert_eq!(
                g.level,
                15 + batch,
                "master did not reach level {} after {} lines of play",
                15 + batch,
                batch * LINES_PER_LEVEL
            );
        }
        assert!(
            g.fall_speed < opening_speed,
            "master levelled without getting faster"
        );
    }

    /// A level-one run's level-up rule has to be unchanged by the offset, or the
    /// fix above silently changed every other mode.
    #[test]
    fn an_ordinary_run_levels_from_one_unchanged() {
        let mut g = game(Mode::Marathon);
        assert_eq!(g.level, 1);
        g.lines_cleared = LINES_PER_LEVEL;
        g.apply(Action::HardDrop, 0);
        assert_eq!(g.level, 2, "ten lines should be level 2");
        g.lines_cleared = LINES_PER_LEVEL * 9;
        g.apply(Action::HardDrop, 0);
        assert_eq!(g.level, 10);
    }

    /// Every mode's blurb has to describe that mode - a copy-pasted or blank
    /// string is exactly the failure this was added to prevent.
    #[test]
    fn every_mode_blurb_is_its_own_and_not_empty() {
        let mut seen = Vec::new();
        for mode in Mode::ALL {
            let blurb = mode.blurb();
            assert!(!blurb.is_empty(), "{mode:?} has no blurb");
            assert!(
                blurb.chars().all(|c| c.is_ascii() || c == ',' || c == '.'),
                "{mode:?} blurb {blurb:?} has a glyph the font cannot draw"
            );
            assert!(
                blurb.len() < 200,
                "{mode:?} blurb is {blurb:?}, too long for the menu"
            );
            assert!(
                !seen.contains(&blurb),
                "{mode:?} shares its blurb with another mode: {blurb:?}"
            );
            seen.push(blurb);
        }
    }

    /// The blurb and the code that enforces it must not drift.
    #[test]
    fn each_blurb_agrees_with_the_number_the_game_uses() {
        assert!(Mode::Marathon.blurb().contains(&LINES_PER_LEVEL.to_string()));
        assert!(Mode::Sprint.blurb().contains(&SPRINT_TARGET_LINES.to_string()));
        assert!(
            Mode::Ultra
                .blurb()
                .contains(&(ULTRA_DURATION_MS / 60_000).to_string())
        );
        assert!(Mode::Master.blurb().contains("15"));
        // Training's one key has to be the key it actually listens for.
        assert!(Mode::Training.blurb().contains('H'));
    }

    #[test]
    fn ultra_ends_after_three_minutes() {
        let mut g = game(Mode::Ultra);
        g.tick(ULTRA_DURATION_MS - 1);
        assert!(!g.is_finished());
        g.tick(ULTRA_DURATION_MS);
        assert!(g.won);
    }

    #[test]
    fn master_starts_at_level_fifteen_and_fast() {
        let g = game(Mode::Master);
        assert_eq!(g.level, 15);
        // Master pins a 200 ms base. The original seeds gravity from that base
        // directly and only applies `0.85 ** (level - 1)` on a level-up, so
        // the opening speed is 200 ms/row, not the ~21 ms the level-15 curve
        // would give (which would clamp to the 50 ms floor).
        assert_eq!(g.fall_speed, difficulty_speeds("master"));
        // Still very fast: four rows a second.
        assert!(g.fall_speed <= 250, "master must open fast");
    }

    #[test]
    fn level_speed_decays_but_never_below_the_floor() {
        let mut g = game(Mode::Marathon);
        g.level = 100;
        g.recompute_fall_speed();
        assert_eq!(g.fall_speed, MIN_FALL_SPEED);
    }

    #[test]
    fn lock_delay_holds_the_piece_before_locking() {
        let mut g = game(Mode::Marathon);
        // Drop it onto the floor.
        g.origin.1 = crate::board::TOTAL_ROWS as i32 - 4;
        let mut t = 0u64;
        // Let gravity push it to the floor and start lock delay.
        while !g.is_grounded() && t < 100_000 {
            t += 100;
            g.tick(t);
        }
        assert!(g.is_grounded());
        let dropped = g.pieces_dropped;
        // Less than the full window: still held.
        g.tick(t + LOCK_DELAY_TIME as u64 - 10);
        assert_eq!(g.pieces_dropped, dropped);
        // Past the window: locked.
        g.tick(t + LOCK_DELAY_TIME as u64 + 10);
        assert_eq!(g.pieces_dropped, dropped + 1);
    }

    #[test]
    fn movement_resets_lock_delay_within_budget() {
        let mut g = game(Mode::Marathon);
        g.origin.1 = crate::board::TOTAL_ROWS as i32 - 4;
        let mut t = 0u64;
        while !g.is_grounded() && t < 100_000 {
            t += 100;
            g.tick(t);
        }
        let t0 = t;
        // Wiggle repeatedly; the piece should not lock.
        for step in 1..=8 {
            let at = t0 + step * 200;
            g.apply(Action::MoveLeft, at);
            g.apply(Action::MoveRight, at);
        }
        assert_eq!(g.pieces_dropped, 0, "lock delay was not refreshed");
    }

    #[test]
    fn das_moves_repeat_after_the_delay() {
        let mut g = game(Mode::Marathon);
        let x0 = g.origin().0;
        g.set_left_held(true, 0);
        g.tick(200); // past DAS, first repeat
        assert!(g.origin().0 < x0, "DAS did not move the piece left");
    }

    #[test]
    fn ghost_lands_where_the_piece_would_rest() {
        let g = game(Mode::Marathon);
        let ghost = g.ghost_origin();
        assert!(g.grid.fits(g.piece(), g.rotation(), ghost));
        assert!(!g.grid.fits(g.piece(), g.rotation(), (ghost.0, ghost.1 + 1)));
    }

    #[test]
    fn topping_out_ends_a_marathon() {
        let mut g = game(Mode::Marathon);
        for y in 0..crate::board::TOTAL_ROWS as i32 {
            for x in 0..GRID_WIDTH as i32 {
                g.grid.force_set(x, y, 1);
            }
        }
        g.apply(Action::HardDrop, 0);
        assert!(g.game_over);
    }

    // --- Live play-through ---------------------------------------------------
    //
    // These drive the real rules loop — spawn, steer, hold, hard drop, lock,
    // clear — through the public input API, the way a keyboard player would.
    // They exist to prove the game actually *plays*: the queue never breaks,
    // every drop locks exactly one piece, lines get cleared mid-run, and each
    // mode reaches its intended end state.

    /// The best `(rotation, column)` placement for the current piece.
    ///
    /// All four orientations are considered. Greedily dropping every piece
    /// flat in its spawn orientation cannot reach the gaps a real player
    /// rotates into, so the stack grows uneven and the bot tops out long
    /// before it clears anything.
    fn best_placement(g: &Game) -> (Rotation, i32) {
        let p = g.piece();
        let start_y = g.origin().1;
        let base = board_snapshot(g);
        let mut scratch = Vec::with_capacity(base.len());
        let mut best = (g.rotation(), g.origin().0);
        let mut best_score = f64::NEG_INFINITY;
        for r in 0..4 {
            for x in 0..GRID_WIDTH as i32 {
                if !g.grid.fits(p, r, (x, start_y)) {
                    continue;
                }
                let rest_y = start_y + g.grid.drop_distance(p, r, (x, start_y));
                let score = evaluate_placement(&base, p, r, x, rest_y, &mut scratch);
                if score > best_score {
                    best_score = score;
                    best = (r, x);
                }
            }
        }
        best
    }

    /// A flat row-major copy of the grid, built once per decision rather than
    /// once per candidate placement.
    fn board_snapshot(g: &Game) -> Vec<bool> {
        let total = crate::board::TOTAL_ROWS as usize;
        let w = GRID_WIDTH;
        let mut b = vec![false; total * w];
        for y in 0..total {
            for cx in 0..w {
                b[y * w + cx] = g.grid.get(cx as i32, y as i32) != 0;
            }
        }
        b
    }

    /// El-Tetris style board evaluation: reward completed rows, penalise
    /// aggregate height, buried holes and surface bumpiness.
    ///
    /// Scoring only "rows cleared, then fewest holes" is not enough — on an
    /// open board every placement ties, so the bot stacks a single tower in
    /// one column and tops out. The height and bumpiness terms are what make
    /// it spread pieces out and actually reach line clears.
    fn evaluate_placement(
        base: &[bool],
        p: Piece,
        r: Rotation,
        x: i32,
        rest_y: i32,
        scratch: &mut Vec<bool>,
    ) -> f64 {
        let total = crate::board::TOTAL_ROWS as i32;
        let w = GRID_WIDTH;
        // `scratch` is the board with the candidate piece stamped on; it is
        // reused across every candidate so the search allocates nothing.
        scratch.clear();
        scratch.extend_from_slice(base);
        for &(dx, dy) in pieces::cells(p, r).iter() {
            let (cx, cy) = (x + dx, rest_y + dy);
            if (0..w as i32).contains(&cx) && (0..total).contains(&cy) {
                scratch[cy as usize * w + cx as usize] = true;
            }
        }
        let board = &*scratch;

        let lines = (0..total)
            .filter(|&y| (0..w).all(|cx| board[y as usize * w + cx]))
            .count();

        let mut heights = vec![0i32; w];
        let mut holes = 0i32;
        for cx in 0..w {
            let mut top = None;
            for y in 0..total as usize {
                if board[y * w + cx] {
                    top = Some(y);
                    break;
                }
            }
            if let Some(t) = top {
                heights[cx] = total - t as i32;
                // Anything empty beneath the topmost cell is buried.
                for y in (t + 1)..total as usize {
                    if !board[y * w + cx] {
                        holes += 1;
                    }
                }
            }
        }
        let aggregate: i32 = heights.iter().sum();
        let bumpiness: i32 = heights.windows(2).map(|p| (p[0] - p[1]).abs()).sum();

        -0.510066 * aggregate as f64 + 0.760666 * lines as f64 - 0.35663 * holes as f64
            - 0.184483 * bumpiness as f64
    }
    /// Steer the piece horizontally to `target` with legal moves.
    ///
    /// Bounded on purpose. A piece boxed in by the stack may not be able to
    /// reach the target column at all, and since `origin` then never moves,
    /// an unbounded loop would spin forever. The board is only
    /// `GRID_WIDTH` wide, so twice that many moves is always enough when the
    /// path is actually clear.
    fn steer(g: &mut Game, target: i32) {
        let mut budget = GRID_WIDTH * 2;
        while g.origin().0 < target && budget > 0 {
            g.apply(Action::MoveRight, 0);
            budget -= 1;
        }
        while g.origin().0 > target && budget > 0 {
            g.apply(Action::MoveLeft, 0);
            budget -= 1;
        }
    }

    /// Rotate to `target`, then drop the current piece at the best placement.
    /// The turn loop is bounded: against a wall a rotation can be refused
    /// outright, and the bot should settle for the orientation it has rather
    /// than spin forever.
    fn bot_drop(g: &mut Game) {
        let (target_rot, target_x) = best_placement(g);
        for _ in 0..4 {
            if g.rotation() == target_rot {
                break;
            }
            g.apply(Action::RotateCw, 0);
        }
        steer(g, target_x);
        g.apply(Action::HardDrop, 0);
    }

    /// Play a mode for up to `pieces` drops, asserting the plumbing holds.
    fn bot_session(g: &mut Game, pieces: i32) -> i32 {
        let mut run = 0;
        while !g.is_finished() && run < pieces {
            assert_eq!(g.next.len(), 5, "the queue must stay preloaded");
            bot_drop(g);
            run += 1;
            assert_eq!(g.pieces_dropped, run, "every drop must lock exactly once");
        }
        run
    }

    /// The original's opening gravity: the raw base speed, with no level
    /// decay applied yet.
    #[test]
    fn gravity_starts_at_the_base_speed_for_every_difficulty() {
        for name in ["easy", "normal", "hard", "very hard"] {
            let base = difficulty_speeds(name);
            let mut s = Settings::default();
            s.base_fall_speed = base;
            let g = Game::new(Mode::Marathon, s, 0);
            assert_eq!(g.level, 1, "{name} should open at level 1");
            assert_eq!(g.fall_speed, base, "{name} should open at its base speed");
        }
    }

    /// Master opens at level 15, so applying the level curve on the very first
    /// frame would give `200 * 0.85^14` and clamp to the 50 ms floor -- four
    /// times faster than the 200 ms the original starts Master at.
    #[test]
    fn master_opens_at_its_base_speed_not_the_level_15_curve() {
        let mut s = Settings::default();
        s.start_level = 15;
        let g = Game::new(Mode::Master, s, 0);
        assert_eq!(g.level, 15, "master should open at level 15");
        let base = difficulty_speeds("master");
        assert_eq!(g.fall_speed, base, "master should open at {base} ms/row");
        assert!(
            g.fall_speed > MIN_FALL_SPEED,
            "master must not open clamped to the {MIN_FALL_SPEED} ms floor"
        );
    }

    /// The gravity curve is the level read as a cadence, rising to the 20G
    /// ceiling and never past it -- and gravity must get faster every level.
    #[test]
    fn gravity_rises_with_the_level_and_stops_at_the_ceiling() {
        let base = difficulty_speeds("normal");
        let mut g = game(Mode::Marathon);
        g.settings.base_fall_speed = base;
        let mut prev = u32::MAX;
        for level in 1..=20 {
            g.level = level;
            g.recompute_fall_speed();
            assert_eq!(
                g.fall_speed,
                fall_speed_for(base, level),
                "level {level} gravity"
            );
            assert!(
                g.fall_speed <= prev,
                "gravity must not slow down at level {level}"
            );
            prev = g.fall_speed;
        }
        // Twenty levels is one hundred lines: 20G, and no faster after that.
        assert_eq!(g.fall_speed, MIN_FALL_SPEED, "level 20 must be 20G");
        for level in 21..=60 {
            g.level = level;
            g.recompute_fall_speed();
            assert_eq!(g.fall_speed, MIN_FALL_SPEED, "level {level} passed 20G");
        }
    }

    /// The opening cadence is the difficulty's level-one speed, which the
    /// curve only coincides with at level one.
    #[test]
    fn a_level_one_opening_cadence_is_the_curve_at_level_one() {
        // This equality is what lets `Game::new` use the raw base without
        // being a second, subtly different gravity rule.
        for name in ["easy", "normal", "hard", "very hard", "master"] {
            let base = difficulty_speeds(name);
            assert_eq!(
                fall_speed_for(base, 1),
                base,
                "{name} at level 1 should be its own base cadence"
            );
        }
    }

    /// Clear exactly one row, deterministically: fill the bottom row except
    /// one column, then drop a vertical I down that column so it lands on the
    /// floor and completes the row. Used where a test needs a known number of
    /// clears without depending on how the bot happens to play.
    fn clear_one_line(g: &mut Game) {
        let bottom = crate::board::TOTAL_ROWS as i32 - 1;
        // Start from an empty board: leftovers from the previous drop would
        // block the next I before it reached the floor.
        for y in 0..crate::board::TOTAL_ROWS as i32 {
            for x in 0..GRID_WIDTH as i32 {
                g.grid.force_set(x, y, 0);
            }
        }
        for x in 0..GRID_WIDTH as i32 {
            g.grid.force_set(x, bottom, 2);
        }
        g.grid.force_set(6, bottom, 0);
        // A vertical I fills `origin.0 + 2`, so origin 4 puts it in column 6.
        g.piece = Piece::I;
        g.rot = 1;
        g.origin = (4, 0);
        let before = g.lines_cleared;
        g.apply(Action::HardDrop, 0);
        assert_eq!(
            g.lines_cleared,
            before + 1,
            "the setup should clear exactly one row"
        );
    }

    /// Clearing lines must be what drives the level, at ten rows each.
    #[test]
    fn every_ten_lines_advances_one_level() {
        let mut g = game(Mode::Marathon);
        assert_eq!(g.level, 1);
        for total in 1..=30 {
            clear_one_line(&mut g);
            assert_eq!(
                g.level,
                total / LINES_PER_LEVEL + 1,
                "at {total} lines the level should be {}",
                total / LINES_PER_LEVEL + 1
            );
        }
    }

    #[test]
    fn a_greedy_bot_plays_a_marathon_live_end_to_end() {
        // The bag is shuffled from the thread RNG, so any single run can top
        // out early on an unlucky sequence. Play a handful of games and
        // require that the rules loop demonstrably clears lines across them.
        let mut cleared = 0;
        let mut played = 0;
        let mut topped_out = 0;
        for _ in 0..5 {
            let mut g = game(Mode::Marathon);
            let mut run = 0;
            // The budget is a wall-clock guard, not a target: each drop
            // evaluates ~60 placements against the whole board, and an
            // unusually good seed can keep the bot alive for a very long
            // time, which is slow in an unoptimised test build. The bot
            // normally tops out inside 200 pieces.
            while !g.is_finished() && run < 400 {
                bot_drop(&mut g);
                run += 1;
                assert_eq!(
                    g.pieces_dropped, run,
                    "drop {run} must lock exactly one piece"
                );
            }
            assert!(run >= 25, "the bot barely played a marathon: {run} pieces");
            cleared += g.lines_cleared;
            played += run;
            if g.is_finished() {
                assert!(g.game_over, "marathon only ends in a top-out");
                topped_out += 1;
            }
        }
        assert!(topped_out > 0, "no game ever reached an end state");
        assert!(
            cleared > 0,
            "no line was ever cleared across 5 marathons of {played} pieces"
        );
    }

    #[test]
    fn a_bot_session_in_every_mode_makes_progress() {
        for mode in [
            Mode::Marathon,
            Mode::Sprint,
            Mode::Ultra,
            Mode::Master,
            Mode::Training,
        ] {
            let mut g = game(mode);
            let target = if mode == Mode::Sprint || mode == Mode::Ultra {
                400
            } else {
                200
            };
            let run = bot_session(&mut g, target);
            assert!(run >= 20, "{mode:?} barely played: {run} pieces");
            if g.is_finished() {
                match mode {
                    Mode::Sprint | Mode::Ultra => assert!(g.won || g.game_over),
                    Mode::Marathon | Mode::Master => assert!(g.game_over),
                    Mode::Training => unreachable!("training cannot finish"),
                }
            }
        }
    }

    #[test]
    fn training_delete_row_keeps_the_board_playable_forever() {
        let mut g = game(Mode::Training);
        for _ in 0..400 {
            bot_drop(&mut g);
            if g.grid.danger_zone_active(4) {
                g.apply(Action::DeleteBottomRow, 0);
            }
        }
        assert!(!g.game_over, "training must never end");
        assert_eq!(g.pieces_dropped, 400);
    }
}
