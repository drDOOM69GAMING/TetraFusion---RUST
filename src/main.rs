//! TetraFusion - a Rust port of drDOOM69GAMING's Pygame original.
//!
//! The original is a single 3,600-line procedural Pygame script. This port
//! keeps the rules exact - the same kick tables, scoring and speed curve -
//! and restructures the plumbing into explicit state. See README.md for the
//! three gameplay bugs that were fixed along the way.

mod assets;
mod audio;
mod backgrounds;
mod board;
mod config;
mod effects;
mod folderpick;
mod game;
mod keys;
mod music_dir;
mod pad;
mod pieces;
mod render;
mod scores;
mod settings;
mod skins;
mod userdir;

use raylib::consts::GamepadAxis;
use raylib::consts::GamepadButton;
use raylib::consts::KeyboardKey as Key;
use raylib::core::color::Color;
use raylib::prelude::*;

use rand::Rng;

use game::{Action, Event, Game, Mode, Settings};
use effects::Direction;
use pad::{Binding, NavAction, PadAction};

/// Key hints on the game-over screen. Every key that does something is named,
/// because the screen used to accept three of them and say nothing, so the only
/// way to discover them was to lose a run and guess.
const OVER_HINTS: &[&str] = &[
    "ENTER or SPACE - PLAY AGAIN",
    "P - PLAY AGAIN",
    "ESC - MAIN MENU",
    "R - RESTART IN THIS MODE",
];

/// The same hints on the win screen, so a player who has seen one has seen the
/// other and does not have to re-read them.
const WIN_HINTS: &[&str] = OVER_HINTS;

/// Key hints on the initials prompt.
///
/// It has to say that letters are what is wanted. Every other screen in the game
/// takes a handful of named keys, so a player arriving here from a game-over
/// screen has every reason to press Enter immediately - and an empty entry
/// refuses to save, which without this line looks like the game being stuck.
const INITIALS_HINTS: &[&str] = &[
    "TYPE UP TO 3 LETTERS",
    "BACKSPACE - DELETE A LETTER",
    "ENTER - SAVE AND PLAY AGAIN",
    "ESC - MAIN MENU",
];

/// What a press on the game-over screen does.
///
/// Split out as a pure function so the key routing is testable. It is easy to
/// write this as a four-branch `if` chain, get it subtly wrong, and have no way
/// to find out except by playing a whole run to a loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverAction {
    /// Start the same mode again.
    PlayAgain,
    /// Abandon the run and go back to the main menu.
    MainMenu,
    /// Edit the initials in place and stay on the prompt.
    Initials,
    /// Nothing pressed.
    None,
}

/// Decide what the game-over screen does with this frame's input.
///
/// `P` - the key that pauses everywhere else in the game - puts the player back
/// into the game. It used to mean "back to the main menu", which made the pause
/// key the one key on this screen that could not resume: a player who finished
/// a run and reached for `P`, as they do at every other pause in the game, was
/// silently doing nothing unless they happened to know `Enter` was the way in.
///
/// `quit` has to be tested *first*, because Escape reports as both `quit` and
/// `pause`. Testing `pause` first would swallow the only key that leaves for
/// good, and a player who had bound Pause to the same button as Back would be
/// trapped on a dead run with no way out of the screen.
fn over_action(quit: bool, pause: bool, restart: bool, confirm: bool) -> OverAction {
    if quit {
        OverAction::MainMenu
    } else if restart || confirm || pause {
        OverAction::PlayAgain
    } else {
        OverAction::None
    }
}

/// What the initials prompt does with one frame of input.
///
/// A separate decision from [`over_action`] because this screen is the one place
/// where letters *are* keys: `R` here is the letter R, not "restart". Folding the
/// two together is exactly how a high-score entry ends up with the player's
/// initials pre-filled from whatever the game-over screen was listening for.
///
/// The rules, and where each comes from:
///
/// - A letter or digit appends, up to three. Raylib's `is_key_pressed` only fires
///   once per physical press, so there is nothing to suppress; repeating would
///   turn one keypress into a full name.
/// - `BACKSPACE` removes the last letter.
/// - `confirm` saves, **but only if something has been typed**. The original
///   guarded this with `if event.key == pygame.K_RETURN and initials`, and it is
///   what stops a record being filed as `---`.
/// - `quit` or `pause` abandons the entry for the main menu, but only on a frame
///   where no letter was typed. Leaving without a name costs the player the
///   record, the same as losing the run outright: nothing is written and the
///   standing record is untouched.
///
/// `typed` is the single character this frame produced, if any. `None` is the
/// common case, and doubles as the signal that something was typed because there
/// is nowhere else for the edit to go.
///
/// The typed letter is tested **before** `quit` and `pause`, and that ordering is
/// load-bearing. `Action::Pause` defaults to `KEY_P`, so `input.pause` is true on
/// every press of `P`; checking `quit || pause` first meant pressing `P` on this
/// screen jumped to the main menu and threw the record away, and the letter `P`
/// could never be typed at all. A keypress that produced a letter is a letter,
/// whatever else that key is bound to in play - this screen's whole job is
/// reading letters, and the one key the hints offer as a way out is Escape, which
/// is `quit` and is not a letter.
fn initials_action(
    typed: Option<char>,
    backspace: bool,
    confirm: bool,
    quit: bool,
    pause: bool,
    empty: bool,
) -> OverAction {
    // First, because a letter is the primary input on this screen and the keys
    // that also mean something else are ordinary letters. `Action::Pause`
    // defaults to `KEY_P`, so the old order - quit, pause, confirm, then type -
    // made `P` abandon the run instead of typing, and the letter P unreachable.
    if typed.is_some() || backspace {
        OverAction::Initials
    } else if quit || pause {
        OverAction::MainMenu
    } else if confirm {
        // An empty entry is not a save. Without this a player could confirm out
        // of the prompt and silently file the score under the blank name.
        if empty {
            OverAction::None
        } else {
            OverAction::PlayAgain
        }
    } else {
        OverAction::None
    }
}

/// The letter or digit the player pressed this frame, if any.
///
/// Raylib's key codes for `A`-`Z` and `0`-`9` are contiguous and equal the ASCII
/// values, so the whole alphabet is one scan with no lookup table.
///
/// Scans rather than calling `get_char_pressed`, which would report the character
/// a key *produced*: unshifted letters come back lowercase, so the name would be
/// lower-cased before `Initials::push` ever saw it. The reverse of this mapping,
/// `scores::key_to_char`, is the tested half.
///
/// A `&RaylibHandle` rather than a generic borrow so this stays callable while
/// `begin_drawing` holds the drawing handle.
fn typed_letter(rl: &raylib::core::RaylibHandle) -> Option<char> {
    // Digits are 48..=57 and letters 65..=90. Both ranges are scanned over
    // together so the whole alphabet is one pass; the punctuation keys that share
    // the space between them (`;`, `[`, `]`, `\`) are not scanned at all, because
    // `key_const_digit`/`key_const_letter` would return a *wrong* key for them
    // rather than nothing.
    //
    // At most one letter can go down per frame in practice, and taking the first
    // match keeps it to one character per frame even if a keyboard reports two.
    (b'0' as i32..=b'9' as i32)
        .chain(b'A' as i32..=b'Z' as i32)
        .map(|code| {
            let key = if (48..=57).contains(&code) {
                key_const_digit(code)
            } else {
                key_const_letter(code)
            };
            (rl.is_key_pressed(key)).then_some(code)
        })
        .find_map(|code| code)
        .and_then(scores::key_to_char)
}

/// The digit key for an ASCII digit code `'0'`..`'9'`.
///
/// Raylib spells these out in words - `KEY_ZERO`, not `KEY_0` - which is why
/// there is a mapping at all. Anything outside the range returns
/// [`Key::KEY_NULL`], which is never pressed.
fn key_const_digit(c: i32) -> Key {
    match c {
        48 => Key::KEY_ZERO,
        49 => Key::KEY_ONE,
        50 => Key::KEY_TWO,
        51 => Key::KEY_THREE,
        52 => Key::KEY_FOUR,
        53 => Key::KEY_FIVE,
        54 => Key::KEY_SIX,
        55 => Key::KEY_SEVEN,
        56 => Key::KEY_EIGHT,
        _ => Key::KEY_NINE,
    }
}

/// The letter key for an ASCII uppercase code `'A'`..`'Z'`.
///
/// Digits aside, raylib's letter key codes are spelled as single characters and
/// are contiguous with the ASCII values, so this is a straight lookup.
/// [`Key::KEY_Z`] is the fallback and covers `'Z'` itself.
fn key_const_letter(c: i32) -> Key {
    match c {
        65 => Key::KEY_A,
        66 => Key::KEY_B,
        67 => Key::KEY_C,
        68 => Key::KEY_D,
        69 => Key::KEY_E,
        70 => Key::KEY_F,
        71 => Key::KEY_G,
        72 => Key::KEY_H,
        73 => Key::KEY_I,
        74 => Key::KEY_J,
        75 => Key::KEY_K,
        76 => Key::KEY_L,
        77 => Key::KEY_M,
        78 => Key::KEY_N,
        79 => Key::KEY_O,
        80 => Key::KEY_P,
        81 => Key::KEY_Q,
        82 => Key::KEY_R,
        83 => Key::KEY_S,
        84 => Key::KEY_T,
        85 => Key::KEY_U,
        86 => Key::KEY_V,
        87 => Key::KEY_W,
        88 => Key::KEY_X,
        89 => Key::KEY_Y,
        _ => Key::KEY_Z,
    }
}

/// Which screen is up.
#[derive(Clone)]
enum Screen {
    Menu,
    Playing,
    Paused { sel: usize },
    Over,
    /// The three-letter initials prompt, shown instead of the game-over screen
    /// when a run sets a new record.
    ///
    /// A run of its own rather than a flag on `Over` because the two screens want
    /// opposite key handling: on `Over` almost any key restarts, while here the
    /// same keys are letters. Sharing one screen would mean either the restart
    /// key typing an `R` or the letters not being typeable.
    Initials,
    Settings { sel: usize },
    /// The Keyboard Keybinds screen, a submenu of Options.
    ///
    /// `capturing` is `Some(action)` while waiting for the player to press the
    /// key they want for that action; every key is ignored for as long as it is
    /// set, so the Enter that opened the capture cannot bind itself.
    Keybinds {
        sel: usize,
        capturing: Option<settings::Action>,
    },
    /// The Controller Keybinds screen, a submenu of Options.
    ///
    /// The controller twin of `Keybinds`, and a separate menu for a real
    /// reason: SDL reports a D-pad as four buttons on a modern pad and as a hat
    /// on an older one, a DualSense reports it as both, and a Switch Pro
    /// controller has no X or Y at all. The binding for an action is therefore
    /// not a fixed button number and has to be something the player can set.
    PadBinds {
        sel: usize,
        capturing: Option<PadAction>,
    },
    /// The Menu Nav Bindings screen, reached from Controller Keybinds.
    ///
    /// The four menu buttons, so the menus can be driven with no keyboard.
    PadNavBinds {
        sel: usize,
        capturing: Option<NavAction>,
    },
}

/// The keybind screen's row count: one per action, plus Back.
const KEYBIND_ROWS: usize = settings::ACTIONS.len() + 1;
const KEYBIND_BACK: usize = settings::ACTIONS.len();

/// The controller keybind screen: the menu-nav submenu, one row per action,
/// then Back. The same shape as the original's, which listed "Menu Nav
/// Bindings" first and "Back to Options" last.
const PAD_BIND_ROWS: usize = PAD_ACTION_SLOTS + 2;
const PAD_NAV_ROW: usize = 0;
const PAD_BACK_ROW: usize = PAD_ACTION_SLOTS + 1;

/// The menu-nav screen: one row per nav action, then Back.
const PAD_NAV_ROWS: usize = PAD_NAV_SLOTS + 1;
const PAD_NAV_BACK_ROW: usize = PAD_NAV_SLOTS;

/// Slots in [`Pad::level`], in [`pad::PAD_ACTIONS`] order.
///
/// Written out as constants rather than looked up by name because the sampler
/// builds its levels positionally; `the_action_and_nav_tables_are_in_the_declared
/// _order` in the tests is what keeps the two from drifting apart.
mod act {
    pub const LEFT: usize = 0;
    pub const RIGHT: usize = 1;
    pub const DOWN: usize = 2;
    pub const ROTATE: usize = 3;
    pub const HARD_DROP: usize = 4;
    pub const HOLD: usize = 5;
    pub const PAUSE: usize = 6;
    pub const SKIP_TRACK: usize = 7;
}

/// The count those indices are sized from.
const PAD_ACTION_SLOTS: usize = pad::PAD_ACTIONS.len();
const PAD_NAV_SLOTS: usize = pad::NAV_ACTIONS.len();

/// Slots in [`Pad::nav_level`], in [`pad::NAV_ACTIONS`] order.
mod nav {
    pub const UP: usize = 0;
    pub const DOWN: usize = 1;
    pub const SELECT: usize = 2;
    pub const BACK: usize = 3;
}

/// Everything the frame's input needs, sampled once before drawing begins.
///
/// raylib 5.x exposes input on the window handle, which is mutably borrowed
/// for the duration of the frame. Sampling up front keeps the input and draw
/// phases separate.
struct Input {
    left: bool,
    right: bool,
    down: bool,
    down_pressed: bool,
    left_held: bool,
    right_held: bool,
    up: bool,
    rotate_ccw: bool,
    hard_drop: bool,
    hold: bool,
    /// Jump to the next custom music track (the original's Skip Track).
    skip_track: bool,
    delete_row: bool,
    confirm: bool,
    pause: bool,
    quit: bool,
    restart: bool,
    /// Controller state, already merged into the fields above.
    ///
    /// The pad's press and held halves are deliberately *not* kept as separate
    /// `pad_*` fields. The game reads held input in two places - the piece trail
    /// and the wind - and the original sourced those from held key state rather
    /// than from whether a move landed, so the held half has to survive the
    /// merge. Folding it into `left_held` and friends does exactly that, and a
    /// DAS/ARR repeat then behaves identically whichever device is driving it.
    ///
    /// The menu actions are the exception and stay separate, because on the
    /// keyboard Up means Rotate in play but Navigate Up in a menu, and because a
    /// pad's Select lands on a face button a player may also have bound to
    /// Rotate - so merging them would make pressing it in a menu rotate whatever
    /// is underneath as well as confirm. `confirm` and `quit` do fold Select and
    /// Back in, though, because those mean exactly what Enter and Escape already
    /// mean and there is nothing left to tell apart.
    nav_up: bool,
    nav_down: bool,
    /// The controller's Rotate binding, which is a gameplay action rather than a
    /// menu navigation. See the note above.
    pad_rotate: bool,
    /// The lowest-numbered controller button that went down this frame, as an
    /// SDL button number, and the first stick side that crossed its threshold.
    ///
    /// Sampled every frame whether or not a rebind is running, not just while
    /// one is, so a screen that *starts* a capture on a press still has that
    /// press to record instead of losing it and waiting for the next one.
    pad_capture_button: Option<i32>,
    pad_capture_axis: Option<Binding>,
}

/// How many analog axes SDL guarantees on a mapped pad: two sticks, two triggers.
///
/// The order is SDL's, and it is what a stored `Binding::Axis` number indexes,
/// so an axis binding resolves straight into [`read_axes`].
const AXES: usize = 6;

/// Which pad is in use, and what it was doing last frame.
///
/// Carried between frames because two things need history the pad cannot report:
/// the stick hysteresis needs to know whether a side was already engaged, and
/// every press edge needs the previous frame's level.
#[derive(Clone, Copy)]
struct PadState {
    /// The slot being read, or `None` when nothing is plugged in.
    slot: Option<i32>,
    /// Each axis side's engaged state last frame, for the hysteresis and the
    /// rebinding capture. `[_][0]` is the negative side, `[_][1]` the positive.
    axis: [[bool; 2]; AXES],
    /// Each action's level last frame, so an edge can be derived.
    action: [bool; PAD_ACTION_SLOTS],
    /// Each menu action's level last frame.
    nav: [bool; PAD_NAV_SLOTS],
}

impl PadState {
    fn new() -> Self {
        Self {
            slot: None,
            axis: [[false; 2]; AXES],
            action: [false; PAD_ACTION_SLOTS],
            nav: [false; PAD_NAV_SLOTS],
        }
    }

    /// Forget everything about the previous device.
    ///
    /// Called when the slot changes, so a stick left pushed on the old pad cannot
    /// leave the piece sliding sideways after a different pad is picked up - and
    /// so a pad unplugged mid-run does not report a phantom release.
    fn reset_for_new_pad(&mut self, slot: Option<i32>) {
        if self.slot != slot {
            *self = Self::new();
            self.slot = slot;
        }
    }
}

/// The controller's contribution to one frame of input.
#[derive(Default)]
struct Pad {
    /// Press edges for the three movement actions, so one press moves once and
    /// DAS takes over from there.
    left: bool,
    right: bool,
    down: bool,
    /// Levels for the same three, which is what feeds DAS and the piece trail.
    left_held: bool,
    right_held: bool,
    down_held: bool,
    /// Press edges for the one-shot actions.
    rotate: bool,
    hard_drop: bool,
    hold: bool,
    pause: bool,
    skip_track: bool,
    /// Press edges for the menu actions.
    nav_up: bool,
    nav_down: bool,
    nav_select: bool,
    nav_back: bool,
    /// The lowest-numbered button that went down this frame, as an SDL button
    /// number. Only the rebinding screens read it, and it is sampled every frame
    /// whether or not a capture is running, so a screen that *starts* a capture
    /// on a press still has that press to record.
    capture_button: Option<i32>,
    /// The first stick side that crossed its threshold this frame, as a binding.
    capture_axis: Option<Binding>,
}

/// SDL button number to raylib button, for every button a binding can name.
///
/// raylib's `GamepadButton` is *not* SDL's numbering - it renumbers the D-pad to
/// its oddly-named `LEFT_FACE_*` values and shifts the triggers - and the enum is
/// a bindgen-generated C type with no way back from a value. So this is the
/// checked stand-in, written out once, exactly as `keys.rs` does it for keycodes.
/// `the_button_table_matches_the_raylib_enum` in the tests compares every entry
/// against raylib's own discriminants, so the table cannot silently drift.
fn button_table() -> &'static [(i32, GamepadButton)] {
    &[
        (0, GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN),
        (1, GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT),
        (2, GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_LEFT),
        (3, GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_UP),
        (4, GamepadButton::GAMEPAD_BUTTON_MIDDLE_LEFT),
        (5, GamepadButton::GAMEPAD_BUTTON_MIDDLE),
        (6, GamepadButton::GAMEPAD_BUTTON_MIDDLE_RIGHT),
        (7, GamepadButton::GAMEPAD_BUTTON_LEFT_THUMB),
        (8, GamepadButton::GAMEPAD_BUTTON_RIGHT_THUMB),
        (9, GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_1),
        (10, GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1),
        (11, GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_UP),
        (12, GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_DOWN),
        (13, GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_LEFT),
        (14, GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_RIGHT),
        // SDL's MISC1 (15) has no raylib equivalent: GLFW's gamepad API does not
        // expose it, so a binding to it is simply always inactive rather than
        // being quietly mapped onto a different button. The rebinding screen
        // never offers it, so it can only arrive from a hand-edited file.
        (pad::LT_BUTTON, GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_2),
        (pad::RT_BUTTON, GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_2),
    ]
}

/// The raylib button for a stored SDL button number, if there is one.
fn raylib_button(sdl: i32) -> Option<GamepadButton> {
    button_table()
        .iter()
        .find(|(n, _)| *n == sdl)
        .map(|(_, k)| *k)
}

/// Is the given SDL button currently down on `slot`?
fn button_down(rl: &RaylibHandle, slot: i32, sdl: i32) -> bool {
    raylib_button(sdl).is_some_and(|k| rl.is_gamepad_button_down(slot, k))
}

/// Is the given SDL button newly pressed on `slot` this frame?
fn button_pressed(rl: &RaylibHandle, slot: i32, sdl: i32) -> bool {
    raylib_button(sdl).is_some_and(|k| rl.is_gamepad_button_pressed(slot, k))
}

/// All six axes, in SDL's order.
///
/// The triggers come back one-sided: SDL reports them as pressure from 0 to 1
/// and raylib passes that through, so a released trigger is 0.0 rather than
/// centred. That matters because a trigger binding compared against a *negative*
/// threshold would read a trigger nobody is touching as "negative" and fire the
/// action constantly.
fn read_axes(rl: &RaylibHandle, slot: i32) -> [f32; AXES] {
    let get = |a| rl.get_gamepad_axis_movement(slot, a).clamp(-1.0, 1.0);
    [
        get(GamepadAxis::GAMEPAD_AXIS_LEFT_X),
        get(GamepadAxis::GAMEPAD_AXIS_LEFT_Y),
        get(GamepadAxis::GAMEPAD_AXIS_RIGHT_X),
        get(GamepadAxis::GAMEPAD_AXIS_RIGHT_Y),
        get(GamepadAxis::GAMEPAD_AXIS_LEFT_TRIGGER).max(0.0),
        get(GamepadAxis::GAMEPAD_AXIS_RIGHT_TRIGGER).max(0.0),
    ]
}

/// Is the D-pad being pushed in the direction `(x, y)`?
///
/// SDL button numbers, because that is what the bindings store: up 11, down 12,
/// left 13, right 14.
///
/// One implementation covers every kind of hat because under raylib they *are*
/// the same thing. GLFW's gamepad API has no hat at all, and SDL folds a real hat
/// into the D-pad buttons before raylib ever sees it - so a pad that reports a
/// hat, a pad with four separate buttons and a DualSense reporting both are all
/// read here, and a `["hat", [x, y]]` binding from a Python settings file means
/// the same thing as the matching D-pad button binding.
fn dpad(rl: &RaylibHandle, slot: i32, x: i32, y: i32) -> bool {
    // A diagonal is not its own binding target, so it resolves to the axis the
    // player pushed further - which is the one that would have moved the piece.
    if x != 0 && y != 0 {
        return if x.abs() >= y.abs() {
            dpad(rl, slot, x, 0)
        } else {
            dpad(rl, slot, 0, y)
        };
    }
    let btn = match (x, y) {
        (0, -1) => 11,
        (0, 1) => 12,
        (-1, 0) => 13,
        (1, 0) => 14,
        _ => return false,
    };
    button_down(rl, slot, btn)
}

/// Is `binding` satisfied by the pad's current state?
///
/// One place for every kind of input, so a D-pad direction bound to Rotate
/// behaves exactly like the D-pad driving Move Left, and a stick direction bound
/// to Hard Drop behaves like a button.
fn binding_active(
    rl: &RaylibHandle,
    slot: i32,
    binding: Option<Binding>,
    axes: &[f32; AXES],
    st: &pad::PadSettings,
) -> bool {
    match binding {
        Some(Binding::Button(b)) => button_down(rl, slot, b),
        Some(Binding::Hat(x, y)) => dpad(rl, slot, x, y),
        Some(Binding::Axis { axis, positive }) => {
            let Some(&v) = usize::try_from(axis).ok().and_then(|i| axes.get(i)) else {
                return false;
            };
            // A trigger is one-sided, so the side half of the binding is ignored
            // there; comparing it against a negative threshold would read a
            // resting trigger as pressed.
            if axis >= 4 {
                v >= st.analog_threshold
            } else {
                st.on_side(v, positive)
            }
        }
        Some(Binding::None) | None => false,
    }
}

/// Resolve every axis side's engaged level, once.
///
/// Both sides of an axis share one hysteresis band, which is what stops the
/// piece sliding while a stick sits between "left" and "right": a stick held
/// slightly off-centre must read as one direction, not as a fresh press on each
/// side every other frame.
fn axis_levels(axes: &[f32; AXES], st: &pad::PadSettings, last: &[[bool; 2]; AXES]) -> [[bool; 2]; AXES] {
    let mut out = [[false; 2]; AXES];
    for (i, pair) in out.iter_mut().enumerate() {
        for (k, positive) in [false, true].into_iter().enumerate() {
            let v = if positive { axes[i] } else { -axes[i] };
            pair[k] = st.engaged(v, last[i][k]);
        }
    }
    out
}

/// Sample the pad for this frame.
///
/// Four passes, in the order a player would expect them to matter:
///
/// 1. the player's own bindings, resolved as levels;
/// 2. the automatic left-stick steering, so someone who has never opened the
///    controller settings can still move with the stick - the original's
///    `JOYAXISMOTION` handler did this unconditionally;
/// 3. the automatic D-pad steering, gated by `use_dpad`;
/// 4. edges, derived from this frame's levels and last frame's.
///
/// Passes 2 and 3 are additive rather than fallbacks. The original let whichever
/// source spoke last overwrite the other's flag, but a player who has bound a
/// face button to Move Left *and* nudges the stick expects both to work, and no
/// input here can reasonably mean two different things.
///
/// Every edge is `now && !last` rather than a per-input "pressed" flag. That is
/// what makes a held D-pad or a held stick move the piece once and then hand over
/// to DAS at the player's own ARR - the feel the keyboard gives, and the feel a
/// controller should give too.
fn sample_pad(rl: &RaylibHandle, cfg: &settings::Settings, state: &mut PadState) -> Pad {
    let st = cfg.controller_settings.sanitized();
    // Re-resolved every frame, which is what makes a controller plugged in after
    // launch work with no restart.
    let slot = pad::resolve_slot(st.pad_slot, |i| rl.is_gamepad_available(i));
    state.reset_for_new_pad(slot);
    let Some(slot) = slot else {
        return Pad::default();
    };

    let axes = read_axes(rl, slot);
    let levels = axis_levels(&axes, &st, &state.axis);

    // --- pass 1: the player's own bindings ------------------------------
    let bound = pad::PAD_ACTIONS.map(|a| {
        binding_active(rl, slot, cfg.controller_controls.get(a), &axes, &st)
    });
    let nav_bound = pad::NAV_ACTIONS.map(|a| {
        binding_active(rl, slot, cfg.controller_menu_navigation.get(a), &axes, &st)
    });

    // --- pass 2: the automatic left stick -------------------------------
    let stick_left = levels[0][0];
    let stick_right = levels[0][1];
    let stick_down = levels[1][1];

    // --- pass 3: the automatic D-pad ------------------------------------
    let dpad_left = st.use_dpad && dpad(rl, slot, -1, 0);
    let dpad_right = st.use_dpad && dpad(rl, slot, 1, 0);
    let dpad_soft = st.use_dpad && dpad(rl, slot, 0, 1);
    // The D-pad also drives the menus, and that is not gated. Turning the
    // automatic piece steering off must not leave a player unable to reach the
    // Controller Keybinds menu to turn it back on.
    let dpad_up = dpad(rl, slot, 0, -1);
    let dpad_menu_down = dpad(rl, slot, 0, 1);

    let mut level = [false; PAD_ACTION_SLOTS];
    level[act::LEFT] = bound[act::LEFT] || stick_left || dpad_left;
    level[act::RIGHT] = bound[act::RIGHT] || stick_right || dpad_right;
    level[act::DOWN] = bound[act::DOWN] || stick_down || dpad_soft;
    level[act::ROTATE] = bound[act::ROTATE];
    level[act::HARD_DROP] = bound[act::HARD_DROP];
    level[act::HOLD] = bound[act::HOLD];
    level[act::PAUSE] = bound[act::PAUSE];
    level[act::SKIP_TRACK] = bound[act::SKIP_TRACK];

    let mut nav_level = [false; PAD_NAV_SLOTS];
    nav_level[nav::UP] = nav_bound[nav::UP] || dpad_up;
    nav_level[nav::DOWN] = nav_bound[nav::DOWN] || dpad_menu_down;
    nav_level[nav::SELECT] = nav_bound[nav::SELECT];
    nav_level[nav::BACK] = nav_bound[nav::BACK];

    // --- pass 4: edges --------------------------------------------------
    let edge = |now: &[bool], last: &[bool]| -> [bool; PAD_ACTION_SLOTS] {
        let mut out = [false; PAD_ACTION_SLOTS];
        for (o, (n, l)) in out.iter_mut().zip(now.iter().zip(last.iter())) {
            *o = *n && !*l;
        }
        out
    };
    let edges = edge(&level, &state.action);
    let nav_edges = edge(&nav_level, &state.nav);

    // --- the rebinding capture ------------------------------------------
    let capture_button = (0..=pad::RT_BUTTON).find(|&b| button_pressed(rl, slot, b));
    let capture_axis = (0..AXES).find_map(|i| {
        let k = usize::from(levels[i][1]);
        (levels[i][k] && !state.axis[i][k]).then(|| Binding::Axis {
            axis: i as i32,
            positive: k == 1,
        })
    });

    state.axis = levels;
    state.action = level;
    state.nav = nav_level;

    Pad {
        left: edges[act::LEFT],
        right: edges[act::RIGHT],
        down: edges[act::DOWN],
        left_held: level[act::LEFT],
        right_held: level[act::RIGHT],
        down_held: level[act::DOWN],
        rotate: edges[act::ROTATE],
        hard_drop: edges[act::HARD_DROP],
        hold: edges[act::HOLD],
        pause: edges[act::PAUSE],
        skip_track: edges[act::SKIP_TRACK],
        nav_up: nav_edges[nav::UP],
        nav_down: nav_edges[nav::DOWN],
        nav_select: nav_edges[nav::SELECT],
        nav_back: nav_edges[nav::BACK],
        capture_button,
        capture_axis,
    }
}

/// Lattice colour for the current theme and grid settings.
///
/// Returns `None` when the lattice should not be drawn at all, which is the
/// original's `grid_lines: False`. `grid_opacity: 0` is equally invisible, so
/// both paths end up skipping the draw rather than emitting fully transparent
/// lines.
fn grid_color(cfg: &settings::Settings) -> Option<Color> {
    if !cfg.grid_lines || cfg.grid_opacity == 0 {
        return None;
    }
    let g = config::THEMES[cfg.theme % config::THEMES.len()].grid;
    Some(Color::new(g[0], g[1], g[2], cfg.grid_opacity))
}

/// Sample this frame's input, from the keyboard and the controller together.
///
/// The remappable keyboard actions come from cfg.controls, so rebinding
/// actually changes how the game plays. Escape stays hard-wired to quit/pause
/// rather than following the Pause binding: binding Pause to something else must
/// not leave the player with no way out of a run.
///
/// The WASD/Z/C/Shift alternates are kept alongside the bound keys so they still
/// work after a rebind, matching what the original's menus lead people to expect.
fn sample(rl: &RaylibHandle, state: &mut PadState, cfg: &settings::Settings) -> Input {
    let up = |k| rl.is_key_pressed(k);
    let down = |k| rl.is_key_down(k);
    let pad = sample_pad(rl, cfg, state);
    let k = |a| cfg.controls.get(a);
    Input {
        left: up(k(settings::Action::Left)) || up(Key::KEY_A) || pad.left,
        right: up(k(settings::Action::Right)) || up(Key::KEY_D) || pad.right,
        down: down(k(settings::Action::Down)) || down(Key::KEY_S) || pad.down_held,
        down_pressed: up(k(settings::Action::Down)) || up(Key::KEY_S) || pad.down,
        left_held: down(k(settings::Action::Left)) || down(Key::KEY_A) || pad.left_held,
        right_held: down(k(settings::Action::Right)) || down(Key::KEY_D) || pad.right_held,
        up: up(k(settings::Action::Rotate)) || up(Key::KEY_W),
        rotate_ccw: up(Key::KEY_Z) || up(Key::KEY_LEFT_CONTROL),
        hard_drop: up(k(settings::Action::HardDrop)) || pad.hard_drop,
        hold: up(k(settings::Action::Hold)) || up(Key::KEY_LEFT_SHIFT) || pad.hold,
        // Only meaningful while a custom playlist is loaded; Audio::skip ignores
        // it otherwise. The controller adds Skip Track where the keyboard has no
        // alternate for it, as in the original.
        skip_track: up(k(settings::Action::SkipTrack)) || pad.skip_track,
        delete_row: up(Key::KEY_H),
        confirm: up(Key::KEY_ENTER) || up(k(settings::Action::HardDrop)) || pad.nav_select,
        pause: up(k(settings::Action::Pause)) || up(Key::KEY_ESCAPE) || pad.pause,
        // Escape, or the Menu Nav "Back" binding. The pad's own Pause binding is
        // deliberately not wired here: Back means back, and a player who has bound
        // Pause to the same face button as Select still has a way out of a run.
        quit: up(Key::KEY_ESCAPE) || pad.nav_back,
        restart: up(Key::KEY_R),
        nav_up: pad.nav_up,
        nav_down: pad.nav_down,
        pad_rotate: pad.rotate,
        pad_capture_button: pad.capture_button,
        pad_capture_axis: pad.capture_axis,
    }
}

/// Import settings and scores written by a build that predates [`userdir`].
///
/// Those builds wrote to the **working directory**, so that is the first place
/// checked. A double-clicked exe normally has its own folder as the working
/// directory, which makes the two the same file; the exe folder is checked as
/// well for the shortcut case where "Start in" points somewhere else.
///
/// Entirely non-fatal: an unreadable file, an unwritable profile or a missing
/// directory all mean the game starts with defaults, which is what it did before
/// there was a per-user folder at all.
fn migrate_legacy_data() {
    let cwd = std::env::current_dir().ok();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf));

    for name in [settings::FILENAME, scores::FILENAME] {
        for dir in [cwd.as_deref(), exe_dir.as_deref()].into_iter().flatten() {
            let legacy = dir.join(name);
            if !legacy.is_file() {
                continue;
            }
            if userdir::migrate_legacy_file(name, &legacy) == userdir::Migration::Migrated {
                eprintln!(
                    "moved {name} into {}",
                    userdir::app_dir().display()
                );
            }
        }
    }
}

fn main() {
    // Import settings and scores left behind by a build that wrote them next to
    // the exe or in the working directory. Done before the loads below, so a
    // player upgrading gets their options and records rather than defaults.
    migrate_legacy_data();

    let mut cfg = settings::Settings::load();
    // The high-score table is loaded once and kept in memory for the session, and
    // written only when a record is actually set. A missing or unreadable file
    // gives an empty table, so this cannot stop the game starting.
    let mut table = scores::Scores::load();
    let mut initials = scores::Initials::new();
    let total_w = config::SCREEN_WIDTH + config::SUBWINDOW_WIDTH;

    let (mut rl, thread) = raylib::init()
        .size(total_w, config::SCREEN_HEIGHT)
        .title("TetraFusion 2.1 - rust edition")
        .resizable()
        .build();
    rl.set_target_fps(60);

    // The window icon. `SetWindowIcon` only has an effect after the window
    // exists, which is why this cannot be part of the `init()` builder above.
    //
    // Non-fatal, like every other asset here: a missing or undecodable file just
    // leaves the default raylib icon and the game carries on.
    //
    // The window icon is *not* set here. It is compiled into the executable's
    // resource section by `build.rs`, because that is the only place Explorer,
    // the taskbar button and Alt-Tab ever look - `SetWindowIcon` cannot reach
    // any of them.
    //
    // There used to be a runtime call here, unwrapping the `.ico` container by
    // hand and handing the pixels to `SetWindowIcon`. It appeared to work and
    // was entirely inert:
    //
    //   * `SetWindowIcon` only replaces the running window's HICON, so the file
    //     in Explorer and the taskbar entry kept their default icon no matter
    //     how well it succeeded.
    //   * raylib's GLFW backend then loads the class icon from the executable's
    //     resources (`LoadImageW(hModule, L"GLFW_ICON", ...)`) and had already
    //     installed the default, and
    //   * the call overwrote that default with a *blank* icon, because GLFW's
    //     Win32 `createIcon` builds a 1-bit AND mask it never writes to and
    //     reads it back as fully transparent.
    //
    // So the only visible effect of the runtime path was replacing a correct
    // icon with an invisible one. `src/ico.rs` went with it.
    //
    // Non-fatal, like every other asset here: a missing or undecodable icon just
    // leaves the default raylib icon and the game carries on.

    // The game is laid out at this fixed size and drawn into an off-screen
    // texture every frame, then scaled to whatever the framebuffer actually
    // is. Without this the window only ever looks right at exactly one size:
    // maximise it or go fullscreen and the UI stays pinned to the top-left
    // corner at its original pixel size with black filling the rest.
    let viewport = (total_w, config::SCREEN_HEIGHT);
    let mut target = rl
        .load_render_texture(&thread, total_w as u32, config::SCREEN_HEIGHT as u32)
        .expect("could not create the game render texture");

    // `TF_WINSIZE=1920x1080` forces the window size, so the scale-to-fit path
    // can be exercised headlessly the way F11/fullscreen exercises it by hand.
    if let Ok(spec) = std::env::var("TF_WINSIZE") {
        if let Some((w, h)) = spec.split_once('x') {
            if let (Ok(w), Ok(h)) = (w.parse::<i32>(), h.parse::<i32>()) {
                if w > 0 && h > 0 {
                    rl.set_window_size(w, h);
                }
            }
        }
    }

    // Per-level background photos, loaded once. Missing or undecodable images
    // are skipped, leaving the plain backdrop.
    let mut backgrounds = backgrounds::Backgrounds::load(&mut rl, &thread);
    if backgrounds.is_empty() {
        eprintln!("background: none available; playing on the plain backdrop.");
    }

    // Audio is fully optional: no device and/or missing assets degrade to
    // silence instead of stopping the game.
    let audio_dev = raylib::audio::RaylibAudio::init_audio_device().ok();
    if audio_dev.is_none() {
        eprintln!("audio: could not open the audio device; running without sound.");
    }
    let mut audio = audio_dev.as_ref().map(|d| audio::Audio::new(d, &cfg));

    // `TF_SMOKE=1` drives a short autonomous Marathon (one hard drop per
    // frame), grabs a screenshot and exits. Used to verify the playfield
    // renders with a visible stack at the floor.
    let smoke = std::env::var("TF_SMOKE").map(|v| v == "1").unwrap_or(false);

    // `TF_MODE=ultra` picks which mode the smoke run drives, so the panel's clock,
    // a mode's opening level and its ending title can all be rendered and captured
    // without a human playing to them. Anything unrecognised is a Marathon rather
    // than a silent no-op: the point of the variable is to change what is on
    // screen, and quietly ignoring a typo would look exactly like the feature
    // having no effect.
    fn smoke_mode() -> Mode {
        match std::env::var("TF_MODE").ok().as_deref() {
            Some("sprint") => Mode::Sprint,
            Some("ultra") => Mode::Ultra,
            Some("training") => Mode::Training,
            Some("master") => Mode::Master,
            _ => Mode::Marathon,
        }
    }

    let mut screen = Screen::Menu;
    let mut selected = 0usize;
    let mut g: Option<Game> = None;
    let mut quit = false;
    let mut shake = 0.0f32;
    // Drives the skins' breathing, so it counts frames rather than reading a
    // clock: the same value every frame means the same pulse everywhere.
    let mut frame: u64 = 0;
    // Set inside the draw block, acted on after it: `begin_drawing` holds a
    // borrow of `rl`, so the screenshot itself has to wait a frame.
    let mut pending_shot = false;
    // Level-up transition, as in the original: the stack flashes through
    // random colours for LEVEL_TRANSITION_MS, re-randomising every
    // LEVEL_FLASH_INTERVAL ms. The drain and the banner ride on the same window,
    // so there is one clock rather than two that can drift out of step.
    let mut level_flash_until: u64 = 0;
    let mut next_flash_at: u64 = 0; // armed at the first frame of a transition
    // Which effect this transition is running, when it started, and the level it
    // arrived at - enough for the frame loop to turn elapsed milliseconds into a
    // `0..=1` progress value and to label the banner.
    //
    // `None` is the sentinel rather than a bare timestamp: the wall clock's own
    // zero is a legitimate start time, so a `u64` sentinel would be ambiguous
    // for exactly one frame at process start.
    let mut level_fx: Option<(config::LevelFx, u64, i32)> = None;
    // How long the panel keeps showing the "TetraFusion!" call-out after a
    // four-line clear, like the original's `tetris_last_flash` window.
    let mut tetris_flash_until: u64 = 0;
    let mut particles = effects::Particles::new(&cfg.effect);
    let mut pad_state = PadState::new();

    if smoke {
        screen = Screen::Playing;
        let m = smoke_mode();
        g = Some(Game::new(m, game_settings(&cfg, m), 0));
    }

    while !rl.window_should_close() && !quit {
        let now = (rl.get_time() * 1000.0) as u64;
        // F11 toggles fullscreen from anywhere, including the menus. The
        // scene is rendered at a fixed size and scaled on the way out, so
        // this changes nothing about how the game is laid out.
        if rl.is_key_pressed(Key::KEY_F11) {
            rl.toggle_fullscreen();
        }
        let input = sample(&rl, &mut pad_state, &cfg);
        let dt = (rl.get_frame_time() as f32).min(0.1);
        shake = (shake - 1.0).max(0.0);

        // The palette and the block material are both stage properties, so both
        // are resolved from the level on screen every frame. Menus have no game
        // yet and use level one, which is the same look a fresh run opens on.
        let level = g.as_ref().map(|x| x.level).unwrap_or(1);
        let palette = config::palette_for_settings(cfg.theme, cfg.palette_mode, level);
        frame += 1;
        let look = skins::Look::new(resolve_skin(cfg.skin, level), frame);

        match screen {
            Screen::Menu => {
                if input.quit {
                    // ESC (or the gamepad back button) on the menu exits the
                    // game for real.
                    quit = true;
                } else if input.up || input.nav_up {
                    selected = (selected + menu_len() - 1) % menu_len();
                } else if input.down_pressed || input.nav_down {
                    // One step per press; holding DOWN must not zip through
                    // the whole list like the soft-drop key does in play.
                    selected = (selected + 1) % menu_len();
                } else if input.confirm {
                    let modes = Mode::ALL.len();
                    if selected < modes {
                        let m = Mode::ALL[selected];
                        g = Some(Game::new(m, game_settings(&cfg, m), now));
                        level_flash_until = 0;
                        tetris_flash_until = 0;
                        screen = Screen::Playing;
                    } else if selected == modes {
                        screen = Screen::Settings { sel: 0 };
                    } else {
                        quit = true;
                    }
                }
            }
            Screen::Playing => {
                // Skip Track: the original's own action, and the only way to
                // move through a custom playlist. It does nothing unless a
                // custom folder is actually loaded, which Audio::skip checks.
                if input.skip_track {
                    if let Some(a) = audio.as_mut() {
                        a.skip();
                    }
                }
                if let Some(game) = g.as_mut() {
                    game.clear_events();
                    if smoke {
                        // Auto-play: drop whatever piece is current.
                        game.apply(Action::HardDrop, now);
                    }

                    // The mouse is inert during play - controls are keyboard
                    // and controller only, as the user wants. Nothing here
                    // reads the cursor, so it can never fight the keys or pad.

                    pump_input(game, &input, now);
                    game.tick(now);

                    // Piece-trail effect: particles spawn at the piece's edge
                    // while it is steered or soft-dropped, exactly like the
                    // original (it never trails during passive gravity). The
                    // original spawned from held input state - steering into
                    // a wall still smokes - so the condition and the left >
                    // right > down direction priority come from the held
                    // keys/stick/mouse-motion state, not from whether a move
                    // actually landed.
                    if particles.active() {
                        // Already merged keyboard and controller by `sample`.
                        let steer_left = input.left_held;
                        let steer_right = input.right_held;
                        let soft = input.down;
                        if steer_left || steer_right || soft {
                            let dir = if steer_left {
                                Direction::Left
                            } else if steer_right {
                                Direction::Right
                            } else {
                                Direction::Down
                            };
                            particles.spawn_trail(game.piece(), game.rotation(), game.origin(), dir);
                        }
                    }

                    // Themed bursts for the gestures themselves: a slide, a
                    // rotation, a slam. The trail above only fires while a key
                    // is *held*, so a single tap and a rotation produced nothing
                    // at all - these fire on the event, which is what actually
                    // happened, and the level sets how hard they throw.
                    //
                    // Every event carries its own pose. Reading `game.origin()`
                    // here instead would be wrong for a hard drop specifically:
                    // by the time this runs the piece has locked and its
                    // successor has already spawned, so the slam would burst at
                    // the ceiling rather than the floor.
                    let level = game.level;
                    if particles.active() {
                        for ev in &game.events {
                            let (at, dir) = match ev {
                                Event::Move { at, dir } | Event::Rotate { at, dir } => {
                                    (*at, Direction::from_delta(*dir))
                                }
                                Event::HardDrop { at, .. } => (*at, Direction::Down),
                                _ => continue,
                            };
                            particles.spawn_gesture(
                                at.piece,
                                at.rot,
                                at.origin,
                                dir,
                                level,
                            );
                        }
                    }

                    // Dust where a hard drop slams in; debris where a clear
                    // empties cells. Both are independent of the Effect
                    // setting, as in the original.
                    let hard_rows = game
                        .events
                        .iter()
                        .find_map(|e| match e {
                            Event::HardDrop { rows, .. } => Some(*rows),
                            _ => None,
                        })
                        .unwrap_or(0);
                    for ev in &game.events {
                        if hard_rows > 0 {
                            if let Event::Lock { at, .. } = ev {
                                particles.spawn_dust(at.piece, at.rot, at.origin, hard_rows);
                            }
                        }
                        if let Event::LineClear { rows, cells, .. } = ev {
                            for (i, &row) in rows.iter().enumerate() {
                                for (x, &c) in cells[i].iter().enumerate() {
                                    if c != 0 {
                                        particles.explode(x as i32, row as i32, palette[c as usize]);
                                    }
                                }
                            }
                        }
                    }

                    // Screen shake from line clears, as in the original
                    // (8 px + 3 px per extra line), applied next frame.
                    let cleared: usize = game
                        .events
                        .iter()
                        .filter_map(|e| match e {
                            Event::LineClear { rows, .. } => Some(rows.len()),
                            _ => None,
                        })
                        .sum();
                    if cleared > 0 {
                        shake = 8.0 + cleared as f32 * 3.0;
                    }

                    // A four-line clear lights the panel's "TetraFusion!" call-out for
                    // TETRIS_FLASH_MS, as the original does with its
                    // `tetris_last_flash` / `tetris_flash_time` window.
                    if game.events.iter().any(|e| {
                        matches!(e, Event::LineClear { rows, .. } if rows.len() == 4)
                    }) {
                        tetris_flash_until = now + config::TETRIS_FLASH_MS;
                    }

                    // Level-up transition (the original's two-second colour
                    // storm across the stack): arm it on a LevelUp event,
                    // then re-randomise the stack's colours every 100 ms,
                    // ending with one final burst when the clock runs out.
                    //
                    // The re-randomisation is half of the effect and is the
                    // original's. The other half is the drain and the banner,
                    // which are new: the hue storm alone leaves the shape of
                    // the board untouched, so there is nothing to look at except
                    // the colour of blocks that have not moved.
                    if let Some(lv) = game.events.iter().find_map(|e| match e {
                        Event::LevelUp(l) => Some(*l),
                        _ => None,
                    }) {
                        level_flash_until = now + config::LEVEL_TRANSITION_MS;
                        next_flash_at = now + config::LEVEL_FLASH_INTERVAL;
                        level_fx = Some((config::stage_fx(lv), now, lv));
                    }
                    if level_flash_until != 0 {
                        if now >= level_flash_until {
                            game.grid.randomize_colors();
                            level_flash_until = 0;
                            tetris_flash_until = 0;
                            level_fx = None;
                        } else if now >= next_flash_at {
                            game.grid.randomize_colors();
                            next_flash_at = now + config::LEVEL_FLASH_INTERVAL;
                        }
                    }
                }
                if input.pause {
                    screen = Screen::Paused { sel: 0 };
                } else if g.as_ref().is_some_and(|g| g.is_finished()) {
                    // A run that beats the record for its mode asks for initials
                    // instead of going straight to the game-over screen, as in
                    // the original. The comparison is `scores.is_record`, which
                    // is the same test `submit` uses, so a player is never asked
                    // for a name for a score that would then be refused.
                    let record = g
                        .as_ref()
                        .is_some_and(|g| table.is_record(g.mode, g.score));
                    screen = if record {
                        Screen::Initials
                    } else {
                        Screen::Over
                    };
                }
            }
            Screen::Paused { sel } => {
                if input.pause {
                    // P, ESC or the gamepad pause button resumes, as in the
                    // original.
                    screen = Screen::Playing;
                } else if input.up || input.nav_up {
                    let next = (sel + PAUSE_OPTIONS - 1) % PAUSE_OPTIONS;
                    screen = Screen::Paused { sel: next };
                } else if input.down_pressed || input.nav_down {
                    let next = (sel + 1) % PAUSE_OPTIONS;
                    screen = Screen::Paused { sel: next };
                } else if input.restart {
                    // R still quick-restarts without opening the menu.
                    let mode = g.as_ref().map(|g| g.mode).unwrap_or(Mode::Marathon);
                    g = Some(Game::new(mode, game_settings(&cfg, mode), now));
                    level_flash_until = 0;
                    tetris_flash_until = 0;
                    screen = Screen::Playing;
                } else if input.confirm {
                    // ENTER (or the gamepad A button) picks the highlighted
                    // option: Resume / Restart / Quit to Menu.
                    let mode = g.as_ref().map(|g| g.mode).unwrap_or(Mode::Marathon);
                    match sel {
                        0 => screen = Screen::Playing,
                        1 => {
                            g = Some(Game::new(mode, game_settings(&cfg, mode), now));
                            level_flash_until = 0;
                            tetris_flash_until = 0;
                            screen = Screen::Playing;
                        }
                        _ => {
                            // Quit to Menu: the current run is abandoned, the
                            // menu starts a fresh game, like the original's
                            // `main_menu()` path.
                            g = None;
                            selected = 0;
                            screen = Screen::Menu;
                        }
                    }
                }
            }
            Screen::Over => match over_action(
                input.quit,
                input.pause,
                input.restart,
                input.confirm,
            ) {
                OverAction::PlayAgain => {
                    let mode = g.as_ref().map(|g| g.mode).unwrap_or(Mode::Marathon);
                    g = Some(Game::new(mode, game_settings(&cfg, mode), now));
                    level_flash_until = 0;
                    tetris_flash_until = 0;
                    level_fx = None;
                    screen = Screen::Playing;
                }
                OverAction::MainMenu => screen = Screen::Menu,
                OverAction::Initials | OverAction::None => {}
            },
            Screen::Initials => {
                // What letter, if any, went down this frame. Polled from the
                // keycodes directly rather than taken from the merged `Input`,
                // because here every letter is a key rather than a bound action.
                //
                // Scanned over the alphanumeric keycode ranges rather than using
                // raylib's `get_char_pressed`, which reports the *character* a
                // key produced and so would hand back lowercase for an unshifted
                // letter, arriving here already past the point where the name is
                // upper-cased. `scores::key_to_char` is the same mapping in
                // reverse, and it is tested; this loop is the half that needs a
                // window.
                let typed = typed_letter(&rl);

                match initials_action(
                    typed,
                    rl.is_key_pressed(Key::KEY_BACKSPACE),
                    input.confirm,
                    input.quit,
                    input.pause,
                    initials.is_empty(),
                ) {
                    OverAction::Initials => {
                        if rl.is_key_pressed(Key::KEY_BACKSPACE) {
                            initials.backspace();
                        } else if let Some(c) = typed {
                            initials.push(c);
                        }
                    }
                    OverAction::PlayAgain => {
                        // Save and restart, as the original did. The write is
                        // best-effort: a failure costs the record, not the run.
                        if let Some(game) = g.as_ref() {
                            if table.submit(game.mode, game.score, initials.as_str()) {
                                table.save();
                            }
                        }
                        let mode = g.as_ref().map(|g| g.mode).unwrap_or(Mode::Marathon);
                        g = Some(Game::new(mode, game_settings(&cfg, mode), now));
                        level_flash_until = 0;
                        tetris_flash_until = 0;
                        level_fx = None;
                        initials = scores::Initials::new();
                        screen = Screen::Playing;
                    }
                    OverAction::MainMenu => {
                        // No save. The record stands and the run is gone, which is
                        // what pressing Escape on a record screen should mean.
                        screen = Screen::Menu;
                    }
                    OverAction::None => {}
                }
            }
            Screen::Settings { sel } => {
                let mut sel = sel;
                let mut leaving = false;
                // Set when this frame pushed a submenu rather than changing a
                // value. The tail below assigns `screen` unconditionally, so
                // without this the keybind push would be thrown away and the
                // keybind screen could never be opened.
                let mut pushed = false;
                if input.quit {
                    // ESC (or the gamepad back button) leaves the settings
                    // screen and saves.
                    cfg.save();
                    leaving = true;
                } else {
                    if input.up || input.nav_up {
                        sel = (sel + SETTINGS_ROWS - 1) % SETTINGS_ROWS;
                    }
                    if input.down_pressed || input.nav_down {
                        sel = (sel + 1) % SETTINGS_ROWS;
                    }
                    if input.confirm {
                        if sel == ROW_KEYBINDS {
                            // A submenu, not a value: leave without saving.
                            screen = Screen::Keybinds {
                                sel: 0,
                                capturing: None,
                            };
                            pushed = true;
                        } else if sel == ROW_PAD_BINDS {
                            // The controller twin of the row above. A submenu,
                            // not a value: leave without saving.
                            screen = Screen::PadBinds {
                                sel: 0,
                                capturing: None,
                            };
                            pushed = true;
                        } else if sel == ROW_MUSIC_DIR {
                            // Ask the desktop for a folder. On Windows this is
                            // the same File Explorer window a player would
                            // open by hand, so every drive, network share and
                            // OneDrive folder is reachable. Backing out leaves
                            // the setting alone; if the platform genuinely has
                            // no dialog there is nothing to fall back to, so the
                            // row simply does not change. See folderpick.
                            if let folderpick::Outcome::Chose(dir) = pick_music_folder(&cfg) {
                                cfg.music_directory = dir.to_string_lossy().into_owned();
                                cfg.save();
                                if let Some(a) = audio.as_mut() {
                                    a.apply_music_settings(&cfg);
                                }
                            }
                        } else if cycle_setting(&mut cfg, sel) {
                            cfg.save();
                            leaving = true;
                        } else {
                            if sel == ROW_EFFECT {
                                particles.set_effect(&cfg.effect);
                            }
                            if sel == ROW_MUSIC {
                                if let Some(a) = audio.as_mut() {
                                    a.set_music_enabled(cfg.music_enabled);
                                }
                            }
                            if sel == ROW_USE_CUSTOM_MUSIC {
                                // Rebuild the playlist straight away so the
                                // toggle is audible immediately rather than
                                // only on the next run, as the original did.
                                if let Some(a) = audio.as_mut() {
                                    a.apply_music_settings(&cfg);
                                }
                            }
                            cfg.save();
                        }
                    }
                }
                if leaving {
                    screen = Screen::Menu;
                } else if !pushed {
                    screen = Screen::Settings { sel };
                }
            }
            Screen::Keybinds { sel, capturing } => {
                let (mut sel, mut capturing) = (sel, capturing);
                if capturing.is_some() {
                    // Capturing: swallow every key, and bind the first one
                    // pressed. Reading the raw keycode (not the sampled
                    // Input) is the whole point - the player may want to bind
                    // a key that is otherwise an action, and the navigation
                    // keys must not leak through.
                    match keys::classify_capture(rl.get_key_pressed()) {
                        keys::Pressed::Nothing => {}
                        keys::Pressed::Cancel => capturing = None,
                        keys::Pressed::Bind(key) => {
                            let action = capturing.unwrap();
                            cfg.controls.set(action, key);
                            cfg.save();
                            capturing = None;
                        }
                    }
                } else {
                    let input = sample(&rl, &mut pad_state, &cfg);
                    if input.up || input.nav_up {
                        sel = (sel + KEYBIND_ROWS - 1) % KEYBIND_ROWS;
                    }
                    if input.down_pressed {
                        sel = (sel + 1) % KEYBIND_ROWS;
                    }
                    if input.quit {
                        screen = Screen::Settings { sel: ROW_KEYBINDS };
                    } else if input.confirm {
                        if sel == KEYBIND_BACK {
                            screen = Screen::Settings { sel: ROW_KEYBINDS };
                        } else {
                            capturing = Some(settings::ACTIONS[sel]);
                        }
                    }
                }
                if matches!(screen, Screen::Keybinds { .. }) {
                    screen = Screen::Keybinds { sel, capturing };
                }
            }
            Screen::PadBinds { sel, capturing } => {
                let (mut sel, mut capturing) = (sel, capturing);
                // Sampled on both paths, including while capturing: the capture
                // reads the pad's press edge out of it, and that edge only exists
                // for the frame the button actually went down.
                let input = sample(&rl, &mut pad_state, &cfg);
                if capturing.is_some() {
                    // Everything else is swallowed while a capture is running,
                    // so the press that opened it cannot bind itself.
                    match pad::classify_capture(
                        input.pad_capture_button,
                        input.pad_capture_axis,
                        cfg.controller_menu_navigation.get(NavAction::Back),
                    ) {
                        pad::Capture::Nothing => {}
                        pad::Capture::Cancel => capturing = None,
                        pad::Capture::Bind(b) => {
                            let action = capturing.unwrap();
                            cfg.controller_controls.set(action, Some(b));
                            cfg.save();
                            capturing = None;
                        }
                    }
                } else if input.quit {
                    screen = Screen::Settings { sel: ROW_PAD_BINDS };
                } else if input.up || input.nav_up {
                    sel = (sel + PAD_BIND_ROWS - 1) % PAD_BIND_ROWS;
                } else if input.down_pressed || input.nav_down {
                    sel = (sel + 1) % PAD_BIND_ROWS;
                } else if input.confirm {
                    if sel == PAD_BACK_ROW {
                        screen = Screen::Settings { sel: ROW_PAD_BINDS };
                    } else if sel == PAD_NAV_ROW {
                        screen = Screen::PadNavBinds { sel: 0, capturing: None };
                    } else {
                        capturing = Some(pad::PAD_ACTIONS[sel - 1]);
                    }
                }
                if matches!(screen, Screen::PadBinds { .. }) {
                    screen = Screen::PadBinds { sel, capturing };
                }
            }
            Screen::PadNavBinds { sel, capturing } => {
                let (mut sel, mut capturing) = (sel, capturing);
                let input = sample(&rl, &mut pad_state, &cfg);
                if capturing.is_some() {
                    match pad::classify_capture(
                        input.pad_capture_button,
                        input.pad_capture_axis,
                        // The Back binding is the escape hatch *and* one of the
                        // four things being bound here. Binding Back to itself is
                        // therefore how you unbind it - the same trick every
                        // rebinding menu needs - and the check below only refuses
                        // when the Back row itself is the one capturing.
                        if capturing == Some(NavAction::Back) {
                            None
                        } else {
                            cfg.controller_menu_navigation.get(NavAction::Back)
                        },
                    ) {
                        pad::Capture::Nothing => {}
                        // Whatever else was pressed while the Back row was
                        // capturing is a real binding, not a cancel.
                        pad::Capture::Cancel => capturing = None,
                        pad::Capture::Bind(b) => {
                            let action = capturing.unwrap();
                            cfg.controller_menu_navigation.set(action, Some(b));
                            cfg.save();
                            capturing = None;
                        }
                    }
                } else if input.quit {
                    screen = Screen::PadBinds { sel: PAD_NAV_ROW, capturing: None };
                } else if input.up || input.nav_up {
                    sel = (sel + PAD_NAV_ROWS - 1) % PAD_NAV_ROWS;
                } else if input.down_pressed || input.nav_down {
                    sel = (sel + 1) % PAD_NAV_ROWS;
                } else if input.confirm {
                    if sel == PAD_NAV_BACK_ROW {
                        screen = Screen::PadBinds { sel: PAD_NAV_ROW, capturing: None };
                    } else {
                        capturing = Some(pad::NAV_ACTIONS[sel]);
                    }
                }
                if matches!(screen, Screen::PadNavBinds { .. }) {
                    screen = Screen::PadNavBinds { sel, capturing };
                }
            }
        }
        // Feed this frame's events to the audio layer before drawing.
        if let Some(a) = audio.as_mut() {
            let over = matches!(screen, Screen::Over);
            let live = matches!(
                screen,
                Screen::Playing | Screen::Paused { .. } | Screen::Over
            );
            a.update(if live { g.as_ref() } else { None }, over);
        }
        // The original pushed trail particles along with the piece's own
        // momentum: -4 px while steering left, +4 while right, +5 down.
        let wind = (
            if input.left_held {
                -4.0
            } else if input.right_held {
                4.0
            } else {
                0.0
            },
            if input.down {
                5.0
            } else {
                0.0
            },
        );
        particles.update(dt, wind);

        let grid = grid_color(&cfg);
        let ghost_a = if cfg.ghost_piece {
            (cfg.ghost_opacity as f32) / 255.0
        } else {
            0.0
        };
        let offset = if cfg.screen_shake && shake > 0.0 {
            let mag = shake * 2.0;
            let mut rng = rand::thread_rng();
            (
                rng.gen_range(-mag..=mag) as i32,
                rng.gen_range(-mag..=mag) as i32,
            )
        } else {
            (0, 0)
        };

        // EndDrawing runs when the draw handle drops, so scope the draw
        // block to release the borrow before the smoke screenshot runs.
        let tetris_flash = matches!(screen, Screen::Playing) && now < tetris_flash_until;
        {
            let t = rl.get_time();
            // Read the real framebuffer size before `begin_drawing` borrows
            // `rl`. Render dimensions (framebuffer pixels) rather than screen
            // dimensions (logical), so a HiDPI display is filled too.
            let fb = (rl.get_render_width(), rl.get_render_height());
            // Read before `begin_drawing` takes `rl` mutably: the pad's name is
            // the one thing on the rebinding screen that cannot be derived from
            // the bindings, and it only exists on the handle.
            let pad_line = matches!(screen, Screen::PadBinds { .. }).then(|| pad_line(&rl, &cfg));

            // The level-up transition, resolved once and shared by the board and
            // the banner. `None` outside a transition, which is every frame of
            // every run except the two seconds after each level-up.
            let fx_progress = || -> f32 {
                level_fx
                    .map(|(_, start, _)| {
                        (now.saturating_sub(start) as f32
                            / config::LEVEL_TRANSITION_MS as f32)
                            .clamp(0.0, 1.0)
                    })
                    .unwrap_or(1.0)
            };
            let fx = level_fx.map(|(kind, _, _)| (kind, fx_progress()));
            let level_banner = level_fx.map(|(_, _, lv)| lv);
            // The same drain the board and the falling piece are using, for the
            // hold and next previews. Resolved here, before `begin_drawing`,
            // because the draw pass is an FnMut closure that cannot borrow
            // `level_fx` mutably.
            let pkeep = fx
                .map(|(kind, t)| config::piece_keep(kind, t))
                .unwrap_or((1.0, 1.0));

            let mut d = rl.begin_drawing(&thread);
            // Clear the real framebuffer too: whatever the scaled game does
            // not cover shows through as the letterbox bars.
            d.clear_background(Color::BLACK);

            d.draw_texture_mode(&thread, &mut target, |mut tm| {
                tm.clear_background(Color::BLACK);

                // Borrowed, not moved: the draw pass is an FnMut
                // closure and MusicDir holds a PathBuf.
                match &screen {
                    Screen::Menu => {
                        let labels = menu_labels();
                        render::draw_menu(&mut tm, selected, &labels, t as f32);
                        // The rule for whatever is highlighted. Five bare mode
                        // names tell a player nothing about which one is a race,
                        // which one is endless and which one never ends - and the
                        // only way to find out was to start a run and lose it.
                        if selected < Mode::ALL.len() {
                            render::draw_menu_blurb(&mut tm, Mode::ALL[selected]);
                        }
                    }
                    Screen::Settings { sel } => {
                        let rows = settings_rows(&cfg, audio.as_ref());
                        render::draw_settings(&mut tm, *sel, &rows, t as f32);
                    }
                    Screen::Keybinds { sel, capturing } => {
                        let rows = keybind_rows(&cfg, *capturing);
                        render::draw_keybinds(&mut tm, *sel, capturing.is_some(), &rows, t as f32);
                    }
                    Screen::PadBinds { sel, capturing } => {
                        let rows = pad_bind_rows(&cfg, *capturing);
                        render::draw_pad_binds(
                            &mut tm,
                            *sel,
                            capturing.is_some(),
                            &rows,
                            pad_line.as_deref().unwrap_or_default(),
                            t as f32,
                        );
                    }
                    Screen::PadNavBinds { sel, capturing } => {
                        let rows = pad_nav_rows(&cfg, *capturing);
                        render::draw_pad_nav_binds(
                            &mut tm,
                            *sel,
                            capturing.is_some(),
                            &rows,
                            t as f32,
                        );
                    }
                    Screen::Playing => {
                        if let Some(game) = g.as_ref() {
                            backgrounds.draw(&mut tm, game.level, cfg.backgrounds_enabled);
                            render::draw_board(&mut tm, game, &palette, ghost_a, offset, grid, look, fx);
                            particles.draw(&mut tm, offset);
                            render::draw_panel(&mut tm, game, total_w, t, &palette, tetris_flash, look, pkeep, now);
                            render::draw_event_flash(&mut tm, game, t as f32);
                            if let Some(lv) = level_banner {
                                let tint = palette[(lv as usize).clamp(1, 7)];
                                render::draw_level_banner(&mut tm, lv, tint, fx_progress());
                            }
                        }
                    }
                    Screen::Paused { sel } => {
                        if let Some(game) = g.as_ref() {
                            backgrounds.draw(&mut tm, game.level, cfg.backgrounds_enabled);
                            render::draw_board(&mut tm, game, &palette, ghost_a, offset, grid, look, fx);
                            particles.draw(&mut tm, offset);
                            render::draw_panel(&mut tm, game, total_w, t, &palette, tetris_flash, look, pkeep, now);
                            render::draw_pause_menu(&mut tm, *sel, t as f32);
                            render::draw_high_score(&mut tm, game.mode, &table);
                        }
                    }
                    Screen::Initials => {
                        if let Some(game) = g.as_ref() {
                            backgrounds.draw(&mut tm, game.level, cfg.backgrounds_enabled);
                            render::draw_board(&mut tm, game, &palette, ghost_a, offset, grid, look, fx);
                            render::draw_panel(&mut tm, game, total_w, t, &palette, tetris_flash, look, pkeep, now);
                            render::draw_initials(
                                &mut tm,
                                &run_summary(game, game.elapsed_ms(now)),
                                &initials,
                                table.get(game.mode),
                                t as f32,
                            );
                        }
                    }
                    Screen::Over => {
                        if let Some(game) = g.as_ref() {
                            backgrounds.draw(&mut tm, game.level, cfg.backgrounds_enabled);
                            render::draw_board(&mut tm, game, &palette, ghost_a, offset, grid, look, fx);
                            render::draw_panel(&mut tm, game, total_w, t, &palette, tetris_flash, look, pkeep, now);
                            // Three-way, not win/lose. Finishing a Sprint is
                            // "SPRINT COMPLETE", Ultra's clock running out is
                            // "TIME UP", and only a top-out is "GAME OVER" - the
                            // original's own distinction, which the boolean
                            // `game.won` had flattened into "YOU WIN" for a
                            // player who did nothing but survive three minutes.
                            let outcome = game.mode.outcome(game.won);
                            let title = outcome.title();
                            let summary = run_summary(game, game.elapsed_ms(now));
                            // Same three lines on a win and a loss. Both screens
                            // answer the same three questions - can I go again,
                            // how do I go again, and how do I get out - so a
                            // player who has seen one has seen the other.
                            let hints: &[&str] = if game.won {
                                WIN_HINTS
                            } else {
                                OVER_HINTS
                            };
                            render::draw_banner(&mut tm, title, Some(&summary), hints, t as f32);
                        }
                    }
                }
            });

            render::present(&mut d, &target, viewport, fb);

            // Flag the capture, but do it out here: `begin_drawing` borrows
            // `rl`, and the screenshot call needs `rl` back. The frame is
            // grabbed at the top of the next iteration, once this one has
            // been presented, which also avoids racing the buffer swap.
            if smoke && g.as_ref().is_some_and(|g| g.pieces_dropped >= 12) {
                pending_shot = true;
            }
        };

        if pending_shot {
            // Dump the exact board so the screenshot can be checked against
            // ground truth. Row 0 is the top (hidden buffer), the floor is
            // the last row; '.' is empty, '1'-'7' are piece colours.
            if let Some(game) = g.as_ref() {
                let mut out = String::new();
                for y in (0..crate::board::TOTAL_ROWS).rev() {
                    for x in 0..config::GRID_WIDTH {
                        let c = game.grid.get(x as i32, y as i32);
                        out.push(if c == 0 { '.' } else { (b'0' + c) as char });
                    }
                    out.push('\n');
                }
                // The panel's own values, for the same reason: a screenshot on a
                // HiDPI display cannot be trusted for layout, so what the side
                // panel actually put on screen is reported as text. `clock` is
                // `None` for a mode that draws no clock, which is the assertion
                // worth making - Marathon showing one would be a bug, and Sprint
                // showing none would be a bigger one.
                let mut panel = format!(
                    "mode={} label={} level={} score={} lines={} pieces={} elapsed={} clock={:?}\n",
                    game.mode.label(),
                    game.mode.blurb(),
                    game.level,
                    game.score,
                    game.lines_cleared,
                    game.pieces_dropped,
                    game.elapsed_ms(now),
                    game.remaining_ms(now),
                );
                panel.push_str(&out);
                let _ = std::fs::write("smoke_grid.txt", panel);
            }
            rl.take_screenshot(&thread, "smoke.png");
            pending_shot = false;
            quit = true;
        }
    }
    // The window closes when RaylibHandle drops.
}

fn menu_len() -> usize {
    Mode::ALL.len() + 2 // modes + Options + Quit
}

/// How many choices the pause menu offers (Resume / Restart / Quit to Menu),
/// mirroring the original's `pause_game` option list.
const PAUSE_OPTIONS: usize = 3;

fn menu_labels() -> Vec<String> {
    let mut v: Vec<String> = Mode::ALL.iter().map(|m| m.label().to_string()).collect();
    v.push("Options".to_string());
    v.push("Quit".to_string());
    v
}

/// Where a Master run opens, as the original's `level = 15 if difficulty ==
/// 'master' else 1` does.
const MASTER_START_LEVEL: i32 = 15;

/// Whether this mode/difficulty pair is Master.
///
/// Two callers ask for Master and they have to agree: `Mode::Master` in the menu
/// and `Difficulty: Master` in Options. Testing only the mode is what let the
/// difficulty option ship a half-master.
fn master_start(cfg: &settings::Settings, mode: Mode) -> bool {
    mode == Mode::Master || cfg.difficulty == "master"
}

fn game_settings(cfg: &settings::Settings, mode: Mode) -> Settings {
    let base = if mode == Mode::Master {
        // Master pins its own gravity; Game::new folds the multiplier in.
        game::difficulty_speeds("master")
    } else {
        let mult = cfg.gravity_multiplier.max(0.1) as f64;
        (game::difficulty_speeds(&cfg.difficulty) as f64 / mult) as u32
    };
    Settings {
        das_ms: cfg.das,
        arr_ms: cfg.arr,
        base_fall_speed: base.max(1),
        gravity_multiplier: cfg.gravity_multiplier,
        // The Traditional palette has to stop the level rotating the palette
        // slot, not just stop the palette itself turning: otherwise the I piece
        // would still change colour every level and the option would be a
        // lie.
        shift_palette: !config::is_traditional(cfg.palette_mode),
        // Master is reachable two ways - the mode, and the `"master"` difficulty -
        // and both have to open at level 15. Taking only the mode's word for it
        // left `Difficulty: Master` at level 1: fast pieces, but level-1 scoring
        // multipliers, level-1 piece colours, and "Level: 1" in the panel.
        start_level: if master_start(cfg, mode) {
            MASTER_START_LEVEL
        } else {
            1
        },
    }
}

/// One-line result shown on the game-over banner.
///
/// `m:ss`, not `{:.2}s`. The finish time is the number a Sprint player is racing
/// and an Ultra player is bounded by, and `180.43s` is both longer to read than
/// `3:00` and a worse description of a run that lasted exactly the three minutes
/// the mode allows.
fn run_summary(g: &Game, elapsed_ms: u64) -> String {
    let secs = elapsed_ms / 1000;
    format!(
        "{}:{:02} - {} lines, {} pieces, best combo x{}, {} t-spins",
        secs / 60,
        secs % 60,
        g.lines_cleared,
        g.pieces_dropped,
        g.max_combo.max(0),
        g.t_spin_count
    )
}

/// Feed sampled input into the game.
///
/// Every field here already has the keyboard and the controller folded together
/// by [`sample`], so there is nothing to merge again - the pad's press edge and
/// its held state arrive on the same fields the keys do. That is what keeps a
/// DAS/ARR repeat identical whichever device is driving it.
fn pump_input(g: &mut Game, input: &Input, now: u64) {
    g.set_left_held(input.left_held, now);
    g.set_right_held(input.right_held, now);

    if input.left {
        g.apply(Action::MoveLeft, now);
    }
    if input.right {
        g.apply(Action::MoveRight, now);
    }
    if input.down {
        g.apply(Action::SoftDrop, now);
    }
    if input.up || input.pad_rotate {
        g.apply(Action::RotateCw, now);
    }
    if input.rotate_ccw {
        g.apply(Action::RotateCcw, now);
    }
    if input.hard_drop {
        g.apply(Action::HardDrop, now);
    }
    if input.hold {
        g.apply(Action::Hold, now);
    }
    if g.mode == Mode::Training && input.delete_row {
        g.apply(Action::DeleteBottomRow, now);
    }
}

// --- Options screen -------------------------------------------------------

/// The original's cycle lists, in the exact orders the Python used.
const DIFFICULTIES: [&str; 5] = ["easy", "normal", "hard", "very hard", "master"];
const GRAVITIES: [f32; 7] = [0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 5.0];
const EFFECTS: [&str; 7] = ["flame", "wind", "water", "ice", "flicker", "matrix", "none"];
const DAS_VALUES: [u32; 5] = [100, 133, 150, 167, 200];
const ARR_VALUES: [u32; 6] = [17, 33, 50, 67, 83, 100];

const ROW_KEYBINDS: usize = 0;
const ROW_PAD_BINDS: usize = 1;
const ROW_DIFFICULTY: usize = 2;
const ROW_THEME: usize = 3;
const ROW_PALETTE: usize = 4;
const ROW_SKIN: usize = 5;
const ROW_GRAVITY: usize = 6;
const ROW_EFFECT: usize = 7;
const ROW_GHOST_PIECE: usize = 8;
const ROW_GHOST_OPACITY: usize = 9;
const ROW_DAS: usize = 10;
const ROW_ARR: usize = 11;
const ROW_SHAKE: usize = 12;
const ROW_BACKGROUNDS: usize = 13;
const ROW_GRID_LINES: usize = 14;
const ROW_GRID_OPACITY: usize = 15;
const ROW_MUSIC: usize = 16;
const ROW_USE_CUSTOM_MUSIC: usize = 17;
const ROW_MUSIC_DIR: usize = 18;
const ROW_BACK: usize = 19;
const SETTINGS_ROWS: usize = 20;

fn onoff(on: bool) -> &'static str {
    if on { "On" } else { "Off" }
}

fn pretty_difficulty(d: &str) -> String {
    d.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn fmt_gravity(m: f32) -> String {
    if (m.fract()).abs() < 0.001 {
        format!("{}x", m as i32)
    } else {
        format!("{m}x")
    }
}

/// The formatted lines the settings screen draws. Every line is centered, so
/// labels embed their value.
/// What the Controller Keybinds row says: how many actions are bound, and which
/// slot the game reads.
///
/// Pure, so the row can be built without a device. The *connected* pad's name is
/// shown on the submenu itself, where the draw phase has the raylib handle, and
/// repeating it here would go stale the moment a controller is plugged in.
///
/// Kept terse deliberately. The options column is 450px wide and this row has
/// the longest value on the screen, so every character here costs a little
/// font size on all eighteen rows.
fn pad_summary(cfg: &settings::Settings) -> String {
    let st = cfg.controller_settings.sanitized();
    let bound = pad::PAD_ACTIONS
        .iter()
        .filter(|a| cfg.controller_controls.get(**a).is_some())
        .count();
    let where_ = if st.pad_slot == 0 {
        "auto".to_string()
    } else {
        format!("slot {}", st.pad_slot)
    };
    format!("{bound}/{} {where_}", pad::PAD_ACTIONS.len())
}

/// The name shown on the piece-skin row.
///
/// Choice `0` means "follow the stage" and `1..=5` pin one of
/// [`skins::ALL_SKINS`]. The offset by one is what leaves room for the
/// automatic option in front of the list rather than making one of the real
/// skins unreachable.
fn skin_label(choice: usize) -> String {
    if choice == 0 {
        "Follows Stage".to_string()
    } else {
        skins::Skin::from_index(choice - 1).name().to_string()
    }
}

/// How many entries the piece-skin row cycles through: automatic plus each skin.
const SKIN_CHOICES: usize = skins::ALL_SKINS.len() + 1;

/// The skin a level is actually drawn in, given the setting.
///
/// The only place the choice is resolved, so the board and the previews cannot
/// end up in different materials.
fn resolve_skin(choice: usize, level: i32) -> skins::Skin {
    if choice == 0 {
        skins::skin_for_level(level)
    } else {
        skins::Skin::from_index(choice - 1)
    }
}

fn settings_rows(cfg: &settings::Settings, audio: Option<&audio::Audio<'_>>) -> Vec<String> {
    let music = match audio {
        None => "Off (no audio device)".to_string(),
        Some(_) if !cfg.music_enabled => "Off".to_string(),
        // The device exists but no track could be decoded, so the row would
        // otherwise claim music is on while nothing plays.
        Some(a) if !a.music_available() => "On (no track loaded)".to_string(),
        // With a custom folder loaded, name the track and its position so the
        // playlist is observable from the menu; the bundled track is a single
        // looping file and does not need a name.
        Some(a) => match a.now_playing() {
            Some((name, at, of)) => format!("On - {name} ({at} of {of})"),
            None => "On".to_string(),
        },
    };
    vec![
        format!("Keyboard Keybinds: {} actions", settings::ACTIONS.len()),
        // Names the connected pad and which slot is being read, so a player with
        // a wheel in slot 0 and a controller in slot 1 can tell which one the
        // game is actually listening to before they start rebinding to the
        // wrong device.
        format!("Controller Keybinds: {}", pad_summary(cfg)),
        format!("Difficulty: {}", pretty_difficulty(&cfg.difficulty)),
        format!("Theme: {}", config::THEMES[cfg.theme % config::THEMES.len()].name),
        format!("Piece Colours: {}", config::palette_mode_name(cfg.palette_mode)),
        format!("Piece Skin: {}", skin_label(cfg.skin)),
        format!("Gravity: {}", fmt_gravity(cfg.gravity_multiplier)),
        format!("Effect: {}", cfg.effect),
        format!("Ghost Piece: {}", onoff(cfg.ghost_piece)),
        format!("Ghost Opacity: {}", cfg.ghost_opacity),
        format!("DAS: {} ms", cfg.das),
        format!("ARR: {} ms", cfg.arr),
        format!("Screen Shake: {}", onoff(cfg.screen_shake)),
        format!("Backgrounds: {}", onoff(cfg.backgrounds_enabled)),
        format!("Grid Lines: {}", onoff(cfg.grid_lines)),
        format!("Grid Opacity: {}", cfg.grid_opacity),
        format!("Music: {music}"),
        format!("Use Custom Music: {}", onoff(cfg.use_custom_music)),
        format!("Music Folder: {}", music_dir_label(cfg)),
        "Back to Main Menu".to_string(),
    ]
}

/// The music-folder row.
///
/// Shows the folder's own name rather than the full path, because the path is
/// far too long for the 450px column and would run into the side panel. The
/// status in brackets says whether it actually holds playable tracks, because
/// "Use Custom Music: On" over a folder of .txt files looks like a bug.
fn music_dir_label(cfg: &settings::Settings) -> String {
    if cfg.music_directory.trim().is_empty() {
        return "(none set)".to_string();
    }
    let dir = cfg.music_directory.trim();
    let name = std::path::Path::new(dir)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.to_string());
    if !std::path::Path::new(dir).is_dir() {
        return format!("{name} (missing)");
    }
    match music_dir::collect_tracks(std::path::Path::new(dir)).len() {
        0 => format!("{name} (no tracks)"),
        1 => format!("{name} (1 track)"),
        n => format!("{name} ({n} tracks)"),
    }
}

/// Where to open the folder picker.
///
/// Prefers the folder already chosen, so re-picking it is a short trip, and
/// falls back to the process's working directory when none is set. A folder
/// that has since been moved or deleted is passed through anyway:
/// `folderpick` drops a non-existent start folder rather than failing.
fn music_folder_start(cfg: &settings::Settings) -> std::path::PathBuf {
    let chosen = cfg.music_directory.trim();
    if chosen.is_empty() {
        std::env::current_dir().unwrap_or_else(|_| ".".into())
    } else {
        std::path::PathBuf::from(chosen)
    }
}

/// Ask the player which folder to play custom music from.
fn pick_music_folder(cfg: &settings::Settings) -> folderpick::Outcome {
    folderpick::pick_folder(&music_folder_start(cfg))
}

/// Rows for the Keyboard Keybinds screen.
///
/// `capturing` turns the matching row into a "press a key" prompt instead of
/// showing the current binding, so the player can see what they are about to
/// overwrite.
fn keybind_rows(cfg: &settings::Settings, capturing: Option<settings::Action>) -> Vec<String> {
    let mut rows = Vec::with_capacity(KEYBIND_ROWS);
    for action in settings::ACTIONS {
        rows.push(match capturing {
            Some(a) if a == action => format!("{}: ...press a key...", action.label()),
            _ => format!("{}: {}", action.label(), keys::name(cfg.controls.get(action))),
        });
    }
    rows.push("Back to Options".to_string());
    rows
}

/// Rows for the Controller Keybinds screen: the menu-nav submenu, the eight
/// actions, then Back.
///
/// `capturing` turns the matching row into a "press a button" prompt instead of
/// showing the current binding, so the player can see what they are about to
/// overwrite rather than losing it silently.
fn pad_bind_rows(cfg: &settings::Settings, capturing: Option<PadAction>) -> Vec<String> {
    let mut rows = Vec::with_capacity(PAD_BIND_ROWS);
    rows.push(format!(
        "Menu Nav Bindings  ({} bound)",
        pad::NAV_ACTIONS
            .iter()
            .filter(|a| cfg.controller_menu_navigation.get(**a).is_some())
            .count()
    ));
    for action in pad::PAD_ACTIONS {
        let bound = Binding::slot_label(cfg.controller_controls.get(action));
        rows.push(match capturing {
            Some(a) if a == action => format!("{}: ...press a button...", action.label()),
            _ => format!("{}: {bound}", action.label()),
        });
    }
    rows.push("Back to Options".to_string());
    rows
}

/// Rows for the Menu Nav Bindings screen.
fn pad_nav_rows(cfg: &settings::Settings, capturing: Option<NavAction>) -> Vec<String> {
    let mut rows = Vec::with_capacity(PAD_NAV_ROWS);
    for action in pad::NAV_ACTIONS {
        let bound = Binding::slot_label(cfg.controller_menu_navigation.get(action));
        rows.push(match capturing {
            Some(a) if a == action => format!("{}: ...press a button...", action.label()),
            _ => format!("{}: {bound}", action.label()),
        });
    }
    rows.push("Back to Controller Keybinds".to_string());
    rows
}

/// The line above the controller rows: which device, in which slot.
///
/// Says plainly when nothing is found, because that is the one case where a
/// player cannot fix it from this screen - they have to plug a controller in -
/// and a menu that just looks unresponsive gives them nothing to go on.
fn pad_line(rl: &RaylibHandle, cfg: &settings::Settings) -> String {
    let st = cfg.controller_settings.sanitized();
    match pad::resolve_slot(st.pad_slot, |i| rl.is_gamepad_available(i)) {
        Some(slot) => {
            // `None` for a name raylib has not filled in yet, which happens for
            // the frame a device is first seen.
            let name = match rl.get_gamepad_name(slot) {
                Some(n) if !n.trim().is_empty() => n,
                _ => "Controller".to_string(),
            };
            format!("reading slot {slot}: {name}")
        }
        None => "no controller found - plug one in".to_string(),
    }
}

/// Advance one setting row. Returns `true` when the row was "Back", i.e. the
/// caller should return to the menu.
fn cycle_setting(cfg: &mut settings::Settings, sel: usize) -> bool {
    match sel {
        // Entering either keybind screen is handled by the caller, not here.
        ROW_KEYBINDS | ROW_PAD_BINDS => return false,
        ROW_DIFFICULTY => {
            let i = DIFFICULTIES.iter().position(|d| *d == cfg.difficulty).unwrap_or(1);
            cfg.difficulty = DIFFICULTIES[(i + 1) % DIFFICULTIES.len()].to_string();
        }
        ROW_THEME => cfg.theme = (cfg.theme + 1) % config::THEMES.len(),
        ROW_PALETTE => cfg.palette_mode = (cfg.palette_mode + 1) % config::PALETTE_MODES,
        ROW_SKIN => cfg.skin = (cfg.skin + 1) % SKIN_CHOICES,
        ROW_GRAVITY => {
            let i = GRAVITIES
                .iter()
                .position(|g| (g - cfg.gravity_multiplier).abs() < 0.001)
                .unwrap_or(2);
            cfg.gravity_multiplier = GRAVITIES[(i + 1) % GRAVITIES.len()];
        }
        ROW_EFFECT => {
            let i = EFFECTS.iter().position(|e| e == &cfg.effect).unwrap_or(0);
            cfg.effect = EFFECTS[(i + 1) % EFFECTS.len()].to_string();
        }
        ROW_GHOST_PIECE => cfg.ghost_piece = !cfg.ghost_piece,
        ROW_GHOST_OPACITY => {
            let mut v = cfg.ghost_opacity as i32 + 32;
            if v > 255 {
                v = 32;
            }
            cfg.ghost_opacity = v as u8;
        }
        ROW_DAS => {
            let i = DAS_VALUES.iter().position(|v| *v == cfg.das).unwrap_or(2);
            cfg.das = DAS_VALUES[(i + 1) % DAS_VALUES.len()];
        }
        ROW_ARR => {
            let i = ARR_VALUES.iter().position(|v| *v == cfg.arr).unwrap_or(2);
            cfg.arr = ARR_VALUES[(i + 1) % ARR_VALUES.len()];
        }
        ROW_SHAKE => cfg.screen_shake = !cfg.screen_shake,
        ROW_BACKGROUNDS => cfg.backgrounds_enabled = !cfg.backgrounds_enabled,
        ROW_GRID_LINES => cfg.grid_lines = !cfg.grid_lines,
        ROW_GRID_OPACITY => {
            // The original stepped by 64, pinned anything over 255 back to 255,
            // and wrapped 255 -> 0, giving the visible / barely / none cycle.
            // The clamp matters: 192 + 64 is 256, and folding that to 0 instead
            // of 255 would skip past full opacity so the player could never
            // get back to it.
            cfg.grid_opacity = if cfg.grid_opacity < 255 {
                (cfg.grid_opacity as u16 + 64).min(255) as u8
            } else {
                0
            };
        }
        ROW_MUSIC => cfg.music_enabled = !cfg.music_enabled,
        ROW_USE_CUSTOM_MUSIC => cfg.use_custom_music = !cfg.use_custom_music,
        // Opening the folder browser is the caller's job, like the keybinds row.
        ROW_MUSIC_DIR => return false,
        ROW_BACK => return true,
        _ => {}
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- the two results screens ----------------------------------------

    /// The whole alphabet has to be typeable on the initials screen.
    ///
    /// This is the test for the `P` bug. `Action::Pause` defaults to `KEY_P`, and
    /// `input.pause` is sampled as `up(k(Action::Pause)) || ESC || pad.pause`, so
    /// pressing `P` made `pause` true. `initials_action` tested `quit || pause`
    /// *before* the typed letter, so `P` jumped to the main menu and threw the
    /// record away - and the letter P could never be entered at all. The player
    /// had no way to file a name containing the twelfth letter.
    ///
    /// Driven from the real defaults rather than from a hand-written `pause:
    /// true`, so a future change to the default keybind cannot quietly reintroduce
    /// it: any letter that happens to be bound to an action fails here.
    #[test]
    fn the_letter_p_is_typeable_even_though_pause_is_bound_to_p() {
        // The bug, precisely. `input.pause` is
        // `up(k(Action::Pause)) || ESC || pad.pause` and `Action::Pause` defaults
        // to `KEY_P`, so pressing P made `pause` true - and the old ordering
        // tested `quit || pause` before the typed letter. P therefore jumped to the
        // main menu *and* discarded the record the player had just earned, and the
        // letter P could never be entered. Checked against the real default rather
        // than a hand-written `pause: true`, so it cannot pass by accident.
        let default = settings::Settings::default();
        assert_eq!(
            default.controls.get(settings::Action::Pause) as i32,
            'P' as i32,
            "this test is about P specifically; the default moved"
        );
        assert_eq!(
            initials_action(Some('P'), false, false, false, true, false),
            OverAction::Initials
        );
    }

    /// The stronger version, and the one that stays true after a rebind.
    ///
    /// Today only `P` was actually stolen - `Hold` is bound to `C` and
    /// `SkipTrack` to `X`, but neither is passed to `initials_action`. Rather than
    /// rely on that remaining true, this asserts the invariant that no letter can
    /// *ever* be stolen by binding Pause to it, since `pause` is the one flag that
    /// discards a record.
    #[test]
    fn no_letter_can_be_stolen_from_the_entry_by_a_pause_rebind() {
        let default = settings::Settings::default();
        let bound: Vec<i32> = settings::ACTIONS
            .iter()
            .map(|a| default.controls.get(*a) as i32)
            .collect();

        for c in ('A'..='Z').chain('0'..='9') {
            // Raylib's A-Z and 0-9 key codes are their ASCII values; that identity
            // is what `typed_letter` scans, so it is the right way to ask "is this
            // character also a bound key?".
            let is_bound = bound.contains(&(c as i32));
            assert_eq!(
                initials_action(Some(c), false, false, false, is_bound, false),
                OverAction::Initials,
                "{c} cannot be typed even with Pause bound to it"
            );
        }
    }

    /// The letters that are *not* bound to anything still type, and the ones that
    /// are do too - so the alphabet is unbroken either way.
    #[test]
    fn the_initials_field_takes_everything_it_is_offered() {
        for c in ['A', 'E', 'Z', '0', '5', '9'] {
            assert_eq!(
                initials_action(Some(c), false, false, false, false, true),
                OverAction::Initials,
                "{c} was rejected"
            );
        }
    }

    /// Escape still leaves, and it still abandons the entry.
    ///
    /// The `P` fix must not have cost the player the documented way out. Escape
    /// is not a letter, so it never collides with typing, and the hint line
    /// promises exactly this.
    #[test]
    fn escape_still_leaves_the_initials_prompt() {
        // `input.pause` is also true for ESC, so both flags set - as they really
        // are at runtime.
        assert_eq!(
            initials_action(None, false, false, true, true, true),
            OverAction::MainMenu
        );
        assert_eq!(
            initials_action(None, false, false, true, true, false),
            OverAction::MainMenu,
            "escape with letters already typed must still abandon"
        );
    }

    /// A pause key that is not a letter still abandons, so a rebind keeps working.
    #[test]
    fn a_rebound_pause_key_still_abandons_the_entry() {
        assert_eq!(
            initials_action(None, false, false, false, true, true),
            OverAction::MainMenu
        );
    }

    /// Backspace edits, and never leaves.
    #[test]
    fn backspace_edits_the_initials_entry() {
        assert_eq!(
            initials_action(None, true, false, false, false, false),
            OverAction::Initials
        );
        // Even with ESC down, editing wins: a stray pause flag cannot delete a
        // letter and throw the run away in the same frame.
        assert_eq!(
            initials_action(None, true, false, true, true, false),
            OverAction::Initials
        );
    }

    /// Enter saves a finished entry and refuses an empty one.
    #[test]
    fn enter_saves_a_name_but_not_an_empty_one() {
        assert_eq!(
            initials_action(None, false, true, false, false, false),
            OverAction::PlayAgain
        );
        assert_eq!(
            initials_action(None, false, true, false, false, true),
            OverAction::None,
            "an empty entry was accepted as a save"
        );
    }

    /// Nothing at all does nothing.
    #[test]
    fn an_idle_frame_on_the_initials_prompt_does_nothing() {
        assert_eq!(
            initials_action(None, false, false, false, false, false),
            OverAction::None
        );
    }

    /// On the game-over screen `R` restarts and `P` plays again - the opposite of
    /// the initials screen, where both are letters. If these two ever agree, the
    /// prompt would be pre-filled from the game-over screen's listeners, which is
    /// the bug the split into two screens exists to prevent.
    #[test]
    fn the_two_results_screens_disagree_about_the_letters_r_and_p() {
        // Game over: any of these restarts.
        for (quit, pause, restart, confirm) in [
            (false, true, false, false),  // P
            (false, false, true, false), // R
            (false, false, false, true), // Enter
        ] {
            assert_eq!(
                over_action(quit, pause, restart, confirm),
                OverAction::PlayAgain,
                "game over did not restart for quit={quit} pause={pause} restart={restart} confirm={confirm}"
            );
        }
        // Initials: the same two keys are letters.
        assert_eq!(
            initials_action(Some('R'), false, false, false, false, true),
            OverAction::Initials
        );
        assert_eq!(
            initials_action(Some('P'), false, false, false, true, true),
            OverAction::Initials
        );
    }

    /// Escape outranks everything on the game-over screen, including a pause key
    /// bound to the same physical input.
    #[test]
    fn escape_outranks_pause_on_the_game_over_screen() {
        assert_eq!(
            over_action(true, true, true, true),
            OverAction::MainMenu
        );
    }

    // --- grid visibility ------------------------------------------------

    #[test]
    fn the_grid_is_drawn_at_the_full_opacity_by_default() {
        let cfg = settings::Settings::default();
        let c = grid_color(&cfg).expect("grid on by default");
        assert_eq!(c.a, 255);
        // The colour must still be the theme's own lattice colour, only the
        // alpha is player-controlled.
        let theme_grid = config::THEMES[cfg.theme % config::THEMES.len()].grid;
        assert_eq!([c.r, c.g, c.b], theme_grid);
    }

    #[test]
    fn a_faint_grid_is_still_drawn() {
        // "barely visible" is the whole reason opacity is a slider and not a
        // boolean - this must not collapse to the off case.
        let mut cfg = settings::Settings::default();
        cfg.grid_opacity = 32;
        let c = grid_color(&cfg).expect("faint grid is still drawn");
        assert_eq!(c.a, 32);
    }

    #[test]
    fn turning_grid_lines_off_hides_the_lattice_entirely() {
        let mut cfg = settings::Settings::default();
        cfg.grid_lines = false;
        assert!(grid_color(&cfg).is_none());
    }

    #[test]
    fn zero_opacity_hides_the_lattice_even_with_lines_enabled() {
        // Both settings mean "invisible" and must agree, otherwise a player who
        // steps opacity round to 0 sees a faint grid they cannot explain.
        let mut cfg = settings::Settings::default();
        cfg.grid_opacity = 0;
        assert!(cfg.grid_lines);
        assert!(grid_color(&cfg).is_none());
    }

    #[test]
    fn the_grid_follows_the_theme() {
        let mut cfg = settings::Settings::default();
        cfg.theme = (cfg.theme + 1) % config::THEMES.len();
        let c = grid_color(&cfg).expect("grid on");
        let theme_grid = config::THEMES[cfg.theme % config::THEMES.len()].grid;
        assert_eq!([c.r, c.g, c.b], theme_grid);
    }

    // --- keybind rows ---------------------------------------------------

    #[test]
    fn every_action_gets_a_row_and_back_is_last() {
        let cfg = settings::Settings::default();
        let rows = keybind_rows(&cfg, None);
        assert_eq!(rows.len(), KEYBIND_ROWS);
        assert_eq!(rows[KEYBIND_BACK], "Back to Options");
        for (i, action) in settings::ACTIONS.iter().enumerate() {
            assert!(
                rows[i].starts_with(action.label()),
                "row {i} should be {} but was {:?}",
                action.label(),
                rows[i]
            );
        }
    }

    #[test]
    fn a_capturing_row_asks_for_a_key_instead_of_showing_the_binding() {
        let cfg = settings::Settings::default();
        let rows = keybind_rows(&cfg, Some(settings::Action::Rotate));
        let capturing = &rows[settings::ACTIONS
            .iter()
            .position(|a| *a == settings::Action::Rotate)
            .unwrap()];
        assert!(capturing.contains("press a key"), "was {capturing:?}");
        // Every other row keeps showing its real binding.
        assert!(rows[0].contains(keys::name(cfg.controls.get(settings::Action::Left)).as_str()));
    }

    #[test]
    fn keybind_rows_reflect_a_rebind() {
        let mut cfg = settings::Settings::default();
        cfg.controls.set(settings::Action::HardDrop, Key::KEY_F);
        let rows = keybind_rows(&cfg, None);
        let i = settings::ACTIONS
            .iter()
            .position(|a| *a == settings::Action::HardDrop)
            .unwrap();
        assert!(rows[i].ends_with("F"), "was {:?}", rows[i]);
    }

    // --- controller button table -----------------------------------------

    /// The pin for the SDL-to-raylib button table.
    ///
    /// raylib's `GamepadButton` is a bindgen-generated C enum, so its variants
    /// have no `TryFrom<i32>` and nothing rejects a stale table entry - a wrong
    /// value would read whichever button SDL happens to number that way, which
    /// on a DualSense is a different physical button than the label claims. These
    /// are the discriminants from raylib 5.5's own `raylib.h`; the table above is
    /// written against them, and this test is what makes that a fact rather than
    /// a hope. If a raylib upgrade renumbers anything, this fails and the table
    /// gets looked at instead of a controller quietly misbehaving.
    #[test]
    fn the_button_table_matches_the_raylib_enum() {
        let expected: &[(i32, GamepadButton)] = &[
            (0, GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN),
            (1, GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT),
            (2, GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_LEFT),
            (3, GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_UP),
            (4, GamepadButton::GAMEPAD_BUTTON_MIDDLE_LEFT),
            (5, GamepadButton::GAMEPAD_BUTTON_MIDDLE),
            (6, GamepadButton::GAMEPAD_BUTTON_MIDDLE_RIGHT),
            (7, GamepadButton::GAMEPAD_BUTTON_LEFT_THUMB),
            (8, GamepadButton::GAMEPAD_BUTTON_RIGHT_THUMB),
            (9, GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_1),
            (10, GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1),
            (11, GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_UP),
            (12, GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_DOWN),
            (13, GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_LEFT),
            (14, GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_RIGHT),
        ];
        for (sdl, rb) in expected {
            let got = raylib_button(*sdl)
                .unwrap_or_else(|| panic!("SDL button {sdl} has no raylib entry"));
            assert_eq!(
                got as i32, *rb as i32,
                "SDL button {sdl} maps to {:?}, expected {:?}",
                got, rb
            );
        }
        // And the two trigger-as-button extensions, which are raylib's
        // synthesised digital triggers rather than anything SDL reports.
        assert_eq!(
            raylib_button(pad::LT_BUTTON),
            Some(GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_2)
        );
        assert_eq!(
            raylib_button(pad::RT_BUTTON),
            Some(GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_2)
        );
        // The table must be exactly this long: a duplicate would make one of the
        // two SDL numbers unreachable, and the first match wins.
        assert_eq!(button_table().len(), expected.len() + 2);
    }

    #[test]
    fn every_sdl_button_a_binding_can_name_resolves_except_misc1() {
        // 0..=15 is SDL's whole `SDL_GameControllerButton` range, plus the two
        // extensions. MISC1 is the one gap: GLFW does not expose it, so a
        // binding to it is permanently inactive rather than remapped onto some
        // other button the player did not press.
        for sdl in (0..=15).chain([pad::LT_BUTTON, pad::RT_BUTTON]) {
            let resolved = raylib_button(sdl).is_some();
            assert_eq!(resolved, sdl != 15, "SDL button {sdl}");
        }
        // And the table ends where the extensions end - anything past that is a
        // number no device can report.
        assert_eq!(raylib_button(pad::RT_BUTTON + 1), None);
    }

    #[test]
    fn the_dpad_bindings_all_resolve_to_real_buttons() {
        // The three D-pad movement defaults plus the four menu-nav directions are
        // seven of the eleven binding slots' worth of D-pad, so a D-pad typo here
        // would break most of the game at once.
        for sdl in [11, 12, 13, 14] {
            assert!(raylib_button(sdl).is_some(), "D-pad button {sdl}");
        }
    }

    #[test]
    fn the_action_and_nav_tables_are_in_the_declared_order() {
        // The sampler builds its levels positionally and `act::` / `nav::` are
        // hand-written indices, so this is the check that keeps the two from
        // drifting. If a constant is ever inserted into PAD_ACTIONS without
        // renumbering, this is what notices.
        let names: Vec<&str> = pad::PAD_ACTIONS.iter().map(|a| a.label()).collect();
        assert_eq!(
            names,
            [
                "Move Left",
                "Move Right",
                "Soft Drop",
                "Rotate",
                "Hard Drop",
                "Hold Piece",
                "Pause",
                "Skip Track",
            ]
        );
        assert_eq!(act::LEFT, 0);
        assert_eq!(act::RIGHT, 1);
        assert_eq!(act::DOWN, 2);
        assert_eq!(act::ROTATE, 3);
        assert_eq!(act::HARD_DROP, 4);
        assert_eq!(act::HOLD, 5);
        assert_eq!(act::PAUSE, 6);
        assert_eq!(act::SKIP_TRACK, 7);

        let nav_names: Vec<&str> = pad::NAV_ACTIONS.iter().map(|a| a.label()).collect();
        assert_eq!(
            nav_names,
            ["Navigate Up", "Navigate Down", "Select / Confirm", "Back / Cancel"]
        );
        assert_eq!(nav::UP, 0);
        assert_eq!(nav::DOWN, 1);
        assert_eq!(nav::SELECT, 2);
        assert_eq!(nav::BACK, 3);
    }

    // --- settings rows --------------------------------------------------

    #[test]
    fn the_settings_screen_has_a_row_for_every_constant() {
        let cfg = settings::Settings::default();
        // `None` stands in for "no audio device", which is the case that needs
        // no window and is the one every row assertion is about.
        let rows = settings_rows(&cfg, None);
        assert_eq!(rows.len(), SETTINGS_ROWS);
        assert_eq!(rows[ROW_BACK], "Back to Main Menu");
        assert_eq!(rows[ROW_MUSIC], "Music: Off (no audio device)");
        for (row, label) in [
            (ROW_KEYBINDS, "Keyboard Keybinds"),
            (ROW_PAD_BINDS, "Controller Keybinds"),
            (ROW_BACKGROUNDS, "Backgrounds"),
            (ROW_GRID_LINES, "Grid Lines"),
            (ROW_GRID_OPACITY, "Grid Opacity"),
            (ROW_USE_CUSTOM_MUSIC, "Use Custom Music"),
            (ROW_MUSIC_DIR, "Music Folder"),
        ] {
            assert!(
                rows[row].starts_with(label),
                "row {row} should be {label} but was {:?}",
                rows[row]
            );
        }
    }

    #[test]
    fn the_controller_row_reports_the_binding_count_and_the_slot() {
        // The options column is 450px wide and this row has the longest value on
        // it, so the wording is pinned: it has to say both things, and stay short
        // enough that the render layer does not have to shrink the whole list to
        // fit it.
        let mut cfg = settings::Settings::default();
        assert_eq!(settings_rows(&cfg, None)[ROW_PAD_BINDS], "Controller Keybinds: 8/8 auto");

        cfg.controller_settings.pad_slot = 2;
        assert_eq!(
            settings_rows(&cfg, None)[ROW_PAD_BINDS],
            "Controller Keybinds: 8/8 slot 2"
        );

        cfg.controller_controls.set(pad::PAD_ACTIONS[3], None);
        assert_eq!(
            settings_rows(&cfg, None)[ROW_PAD_BINDS],
            "Controller Keybinds: 7/8 slot 2"
        );
    }

    #[test]
    fn the_piece_colour_row_offers_adaptive_and_traditional() {
        let mut cfg = settings::Settings::default();
        assert_eq!(
            settings_rows(&cfg, None)[ROW_PALETTE],
            "Piece Colours: Adaptive"
        );
        assert!(!config::is_traditional(cfg.palette_mode));

        assert!(!cycle_setting(&mut cfg, ROW_PALETTE));
        assert!(config::is_traditional(cfg.palette_mode));
        assert_eq!(
            settings_rows(&cfg, None)[ROW_PALETTE],
            "Piece Colours: Traditional"
        );

        // And back, so the row cycles rather than sticking.
        assert!(!cycle_setting(&mut cfg, ROW_PALETTE));
        assert!(!config::is_traditional(cfg.palette_mode));
    }

    /// The skin row has to offer the automatic option *and* every skin, or one
    /// of them is unreachable. Cycling all the way round has to come back to
    /// where it started.
    #[test]
    fn the_piece_skin_row_offers_automatic_and_every_skin() {
        let mut cfg = settings::Settings::default();
        assert_eq!(settings_rows(&cfg, None)[ROW_SKIN], "Piece Skin: Follows Stage");

        let mut seen: Vec<usize> = Vec::new();
        for step in 1..=SKIN_CHOICES {
            assert!(!cycle_setting(&mut cfg, ROW_SKIN));
            assert!(
                !seen.contains(&cfg.skin),
                "the skin row reached {} twice, at step {step}",
                cfg.skin
            );
            seen.push(cfg.skin);
        }
        assert_eq!(seen.len(), SKIN_CHOICES, "the skin row skipped an entry");
        assert_eq!(
            cfg.skin, 0,
            "a full lap of the skin row should land back on automatic"
        );
    }

    /// Every label the skin row can show has to be one the option can actually
    /// resolve, or the menu names a skin that is not being drawn.
    #[test]
    fn every_skin_label_resolves_to_the_skin_it_names() {
        for choice in 0..SKIN_CHOICES {
            let label = skin_label(choice);
            let expected = if choice == 0 {
                skins::skin_for_level(1)
            } else {
                skins::Skin::from_index(choice - 1)
            };
            assert_eq!(
                resolve_skin(choice, 1),
                expected,
                "choice {choice} labelled {label:?}"
            );
        }
        assert_eq!(skin_label(0), "Follows Stage");
        for (i, skin) in skins::ALL_SKINS.iter().enumerate() {
            assert_eq!(skin_label(i + 1), skin.name());
        }
    }

    /// A pinned skin has to stay pinned as the level climbs, or "Piece Skin:
    /// Neon" would quietly become something else at level 20.
    #[test]
    fn a_pinned_skin_survives_the_level_climbing() {
        for (i, skin) in skins::ALL_SKINS.iter().enumerate() {
            let choice = i + 1;
            for level in 1..=60 {
                assert_eq!(resolve_skin(choice, level), *skin, "{skin:?} at {level}");
            }
        }
    }

    /// Automatic has to actually follow the level, or the row is a lie.
    #[test]
    fn the_automatic_skin_follows_the_stage() {
        let mut seen = Vec::new();
        for level in 1..=(skins::LEVELS_PER_SKIN * skins::ALL_SKINS.len() as i32) {
            let s = resolve_skin(0, level);
            if !seen.contains(&s) {
                seen.push(s);
            }
        }
        assert_eq!(seen, skins::ALL_SKINS.to_vec(), "auto did not walk the skins");
    }

    #[test]
    fn the_options_list_renders_at_its_intended_size_with_the_default_settings() {
        // The render layer shrinks the whole column if any row would be clipped,
        // so a row that has quietly grown past the column no longer looks broken
        // - it just makes all twenty rows quietly smaller. This is the test
        // that notices.
        //
        // 12.4px per character at 24pt is raylib's *measured* default-font
        // advance, taken from `MeasureText` on a real window rather than
        // estimated: "Controller Keybinds: 8/8 auto" measures 360px for 29
        // characters. 0.517 of the font size, plus a pixel of slack for the
        // widest glyph in the string.
        const PX_PER_CHAR_AT_24: f32 = 0.517;
        let cfg = settings::Settings::default();
        for row in settings_rows(&cfg, None) {
            // The widest each row ever gets: as drawn, plus the selection
            // markers, which are the four characters `marked_rows` adds.
            let width = (row.chars().count() + 4) as f32 * PX_PER_CHAR_AT_24 * 24.0 + 1.0;
            assert!(
                width <= config::SCREEN_WIDTH as f32,
                "row {row:?} needs {width}px at 24pt, more than the {}px column",
                config::SCREEN_WIDTH
            );
        }
    }

    /// Both routes to Master have to deliver all of it.
    ///
    /// `Mode::Master` and `Difficulty: Master` are two doors to the same room.
    /// Only the mode was checked, so the difficulty option gave 200 ms/row
    /// gravity at **level 1**: fast pieces, but level-1 scoring multipliers,
    /// level-1 piece colours, and "Level: 1" in the side panel. A difficulty
    /// named Master that was not Master.
    #[test]
    fn both_routes_to_master_open_at_level_fifteen() {
        let mut cfg = settings::Settings::default();
        cfg.difficulty = "master".to_string();

        // The difficulty route.
        let s = game_settings(&cfg, Mode::Marathon);
        assert_eq!(s.start_level, MASTER_START_LEVEL);
        assert!(master_start(&cfg, Mode::Marathon));
        assert_eq!(
            s.base_fall_speed,
            game::difficulty_speeds("master"),
            "the master difficulty must use the master base speed"
        );

        // The mode route, with any other difficulty selected.
        let cfg2 = settings::Settings::default();
        let s2 = game_settings(&cfg2, Mode::Master);
        assert_eq!(s2.start_level, MASTER_START_LEVEL);
        assert!(master_start(&cfg2, Mode::Master));

        // Neither route is borrowed for an ordinary setting.
        cfg.difficulty = "normal".to_string();
        for mode in [Mode::Marathon, Mode::Sprint, Mode::Ultra, Mode::Training] {
            assert_eq!(
                game_settings(&cfg, mode).start_level,
                1,
                "{mode:?} opened above level 1 with Difficulty: Normal"
            );
        }
        assert_eq!(game_settings(&cfg2, Mode::Marathon).start_level, 1);
    }

    /// The Master end state has to actually be a level-15 game, not just a
    /// label. `Settings::start_level` is only read by `Game::new`, so this is
    /// where that field is proven to be wired to anything.
    #[test]
    fn a_master_difficulty_run_really_starts_at_level_fifteen() {
        let mut cfg = settings::Settings::default();
        cfg.difficulty = "master".to_string();
        let g = Game::new(
            Mode::Marathon,
            game_settings(&cfg, Mode::Marathon),
            0,
        );
        assert_eq!(g.level, MASTER_START_LEVEL, "the panel would show Level: 1");
        assert_eq!(g.fall_speed, game::difficulty_speeds("master"));
        // And level 15 is where the gravity ramp is already fast, not level 1's.
        assert!(
            g.at_max_gravity() || g.fall_speed <= 200,
            "master should open pinned near the 20G ceiling"
        );
    }

    /// Every difficulty the options row offers has to be a name the speed table
    /// knows. A difficulty that falls through to the default is a row that lies.
    #[test]
    fn every_difficulty_in_the_options_row_has_its_own_speed() {
        let mut seen = Vec::new();
        for name in DIFFICULTIES {
            let speed = game::difficulty_speeds(name);
            assert!(
                !seen.contains(&speed),
                "{name:?} shares the {speed} ms speed of another difficulty"
            );
            seen.push(speed);
        }
        assert_eq!(seen.len(), DIFFICULTIES.len());
        // And each one produces a different game.
        for name in DIFFICULTIES {
            let mut cfg = settings::Settings::default();
            cfg.difficulty = name.to_string();
            assert!(game_settings(&cfg, Mode::Marathon).base_fall_speed > 0);
        }
    }

    /// `MASTER_START_LEVEL` and the blurb have to agree, or the menu advertises a
    /// level the game does not open at.
    #[test]
    fn the_master_blurb_names_the_level_the_game_opens_at() {
        assert!(Mode::Master
            .blurb()
            .contains(&MASTER_START_LEVEL.to_string()));
    }

    /// The mode clock has to read as a clock.
    #[test]
    fn the_mode_clock_reads_in_minutes_and_seconds() {
        // Rolls into minutes at 60, not at 100.
        assert_eq!(render::clock_string_for_test(0), "0:00");
        assert_eq!(render::clock_string_for_test(1_000), "0:01");
        assert_eq!(render::clock_string_for_test(59_999), "0:59");
        assert_eq!(render::clock_string_for_test(60_000), "1:00");
        assert_eq!(render::clock_string_for_test(ULTRAL), "3:00");
        assert_eq!(render::clock_string_for_test(650_000), "10:50");
    }

    const ULTRAL: u64 = 3 * 60 * 1000;

    /// A result summary that can hold a three-minute run without turning into a
    /// wall of digits.
    #[test]
    fn the_result_summary_fits_an_ultra_run() {
        let mut g = Game::new(Mode::Ultra, game_settings(&settings::Settings::default(), Mode::Ultra), 0);
        g.max_combo = 9;
        g.t_spin_count = 4;
        let s = run_summary(&g, config::ULTRA_DURATION_MS);
        assert!(s.starts_with("3:00"), "an ultra summary read {s:?}");
        assert!(
            s.chars().count() < 90,
            "the summary is {} chars wide: {s:?}",
            s.chars().count()
        );
        assert!(s.contains("9"), "{s:?} lost the combo");
    }

    #[test]
    fn backgrounds_toggle_from_the_options_row() {
        let mut cfg = settings::Settings::default();
        assert!(cfg.backgrounds_enabled);
        assert_eq!(settings_rows(&cfg, None)[ROW_BACKGROUNDS], "Backgrounds: On");

        assert!(!cycle_setting(&mut cfg, ROW_BACKGROUNDS));
        assert!(!cfg.backgrounds_enabled);
        assert_eq!(
            settings_rows(&cfg, None)[ROW_BACKGROUNDS],
            "Backgrounds: Off"
        );

        assert!(!cycle_setting(&mut cfg, ROW_BACKGROUNDS));
        assert!(cfg.backgrounds_enabled);
    }

    #[test]
    fn the_music_folder_row_reports_what_is_actually_in_the_folder() {
        // The row is what tells the player whether picking a folder worked, so
        // the three states it can be in are all pinned here.
        let mut cfg = settings::Settings::default();
        assert_eq!(settings_rows(&cfg, None)[ROW_MUSIC_DIR], "Music Folder: (none set)");

        cfg.music_directory = "Z:/definitely/not/here".into();
        assert_eq!(
            settings_rows(&cfg, None)[ROW_MUSIC_DIR],
            "Music Folder: here (missing)"
        );

        cfg.music_directory = ".".into();
        let label = settings_rows(&cfg, None)[ROW_MUSIC_DIR].clone();
        assert!(
            label.starts_with("Music Folder: ") && label.contains("track"),
            "the crate root should report however many tracks it holds, got {label:?}"
        );
    }

    #[test]
    fn grid_opacity_cycles_visible_barely_none_and_back() {
        // Matches the original's `grid_opacity` handling exactly, so the
        // "visible / barely / none" stops land where the original put them.
        let mut cfg = settings::Settings::default();
        let mut seen = vec![cfg.grid_opacity];
        for _ in 0..5 {
            assert!(!cycle_setting(&mut cfg, ROW_GRID_OPACITY));
            seen.push(cfg.grid_opacity);
        }
        assert_eq!(seen, vec![255, 0, 64, 128, 192, 255]);
        // And it keeps cycling rather than sticking at the top.
        assert!(!cycle_setting(&mut cfg, ROW_GRID_OPACITY));
        assert_eq!(cfg.grid_opacity, 0);
    }

    #[test]
    fn cycling_grid_lines_toggles_it() {
        let mut cfg = settings::Settings::default();
        assert!(cfg.grid_lines);
        assert!(!cycle_setting(&mut cfg, ROW_GRID_LINES));
        assert!(!cfg.grid_lines);
        assert!(!cycle_setting(&mut cfg, ROW_GRID_LINES));
        assert!(cfg.grid_lines);
    }

    #[test]
    fn back_is_the_only_row_that_reports_leaving() {
        let mut cfg = settings::Settings::default();
        for row in 0..SETTINGS_ROWS {
            let leaves = cycle_setting(&mut cfg, row);
            assert_eq!(leaves, row == ROW_BACK, "row {row} reported wrong");
        }
    }

    // --- game-over input routing ------------------------------------------
    //
    // This is the one screen where a wrong key is unrecoverable in practice. The
    // player has just lost a run; if the key that gets them back in does not
    // work, the only way forward is to find the window close button. Every
    // assertion below is a way a player could have been stranded.

    /// The reported change: `P` used to mean "back to the menu" here, and
    /// nothing else on this screen did.
    #[test]
    fn p_resumes_on_the_game_over_screen() {
        assert_eq!(over_action(false, true, false, false), OverAction::PlayAgain);
    }

    /// The trap. `sample()` reports Escape as *both* `quit` and `pause`, because
    /// Escape is the pause key everywhere else in the game. Testing `pause`
    /// before `quit` therefore turns Escape into "play again", and a player who
    /// had bound Pause to the same button as Back - which the pad bindings allow
    /// - would have no key that leaves the screen at all.
    #[test]
    fn escape_leaves_rather_than_resuming() {
        assert_eq!(
            over_action(true, true, false, false),
            OverAction::MainMenu,
            "Escape reports as both quit and pause; it must leave"
        );
        assert_eq!(
            over_action(true, true, true, true),
            OverAction::MainMenu,
            "leaving wins over every other key pressed the same frame"
        );
    }

    /// Every advertised key, checked against the hints actually drawn on the
    /// screen. A hint that does nothing, or a key that does something without a
    /// hint, is the same bug in opposite directions.
    #[test]
    fn every_advertised_key_does_what_the_hint_says() {
        for hint in OVER_HINTS {
            let plays = hint.contains("PLAY AGAIN");
            let menu = hint.contains("MAIN MENU");
            // `sample()`'s own key sets, restated here so the hint text and the
            // routing cannot drift apart without a test failing.
            let input = match () {
                _ if hint.contains("ESC") => (true, true, false, false),
                _ if hint.contains("ENTER or SPACE") => (false, false, false, true),
                _ if hint.contains("RESTART") => (false, false, true, false),
                _ if hint.contains('P') => (false, true, false, false),
                _ => panic!("unrecognised hint {hint:?}"),
            };
            let got = over_action(input.0, input.1, input.2, input.3);
            if plays || hint.contains("RESTART") {
                assert_eq!(got, OverAction::PlayAgain, "{hint:?} promised play again");
            } else if menu {
                assert_eq!(got, OverAction::MainMenu, "{hint:?} promised the menu");
            } else {
                panic!("{hint:?} promises nothing this test can check");
            }
        }
    }

    #[test]
    fn an_idle_game_over_screen_does_nothing() {
        assert_eq!(over_action(false, false, false, false), OverAction::None);
    }

    /// The win screen is the same screen with a different title, so it has to
    /// route identically. A player who wins a mode and cannot get back into it
    /// has strictly less reason to be locked out than one who lost.
    #[test]
    fn a_win_screen_routes_exactly_like_a_loss() {
        assert_eq!(WIN_HINTS, OVER_HINTS);
    }
}

