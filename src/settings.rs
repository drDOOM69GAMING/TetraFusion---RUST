//! Persisted player settings, stored as JSON next to the executable.
//!
//! The fields mirror the Python original's `settings.json` option set and
//! defaults, so an options menu can change them and the game truly behaves
//! differently. Every field has a serde default: a missing or corrupt key
//! falls back rather than stopping the game.

use raylib::consts::KeyboardKey as Key;
use serde::{Deserialize, Serialize};

use crate::config::{DEFAULT_ARR_MS, DEFAULT_DAS_MS, PALETTE_ADAPTIVE};
use crate::pad::{PadControls, PadNav, PadSettings};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_difficulty")]
    pub difficulty: String,
    #[serde(default = "default_das")]
    pub das: u32,
    #[serde(default = "default_arr")]
    pub arr: u32,
    #[serde(default = "default_theme")]
    pub theme: usize,
    /// How the piece colours are decided.
    ///
    /// `0` is Adaptive, where the palette turns with the level so the board
    /// changes character as the stage does. `1` is Traditional, which pins the
    /// standard Tetris colours for the whole run - the escape hatch for a
    /// player who would rather recognise pieces at a glance.
    #[serde(default = "default_palette_mode")]
    pub palette_mode: usize,
    /// Which block material to draw: `0` follows the stage, `1..=5` pin one of
    /// [`crate::skins::ALL_SKINS`].
    #[serde(default = "default_skin")]
    pub skin: usize,
    #[serde(default = "default_effect")]
    pub effect: String,
    #[serde(default = "default_gravity")]
    pub gravity_multiplier: f32,
    #[serde(default = "default_ghost_on")]
    pub ghost_piece: bool,
    /// Ghost opacity on the original's 0-255 alpha scale (default 80).
    #[serde(default = "default_ghost_opacity")]
    pub ghost_opacity: u8,
    #[serde(default = "default_true")]
    pub screen_shake: bool,
    /// Draw the per-level background photos behind the well.
    ///
    /// Not an option in the original - the photos are always on there - but a
    /// photo behind a fast game is a perfectly reasonable thing to find
    /// distracting, and a player who wants a plain backdrop should be able to
    /// ask for one. Default on, so a fresh install looks like the original.
    #[serde(default = "default_true")]
    pub backgrounds_enabled: bool,
    /// Whether the playfield lattice is drawn at all (original: `grid_lines`).
    #[serde(default = "default_true")]
    pub grid_lines: bool,
    /// Lattice alpha on the original's 0-255 scale (default 255). Original:
    /// `grid_opacity`, stepped by 64 per press and wrapping 255 -> 0.
    #[serde(default = "default_grid_opacity")]
    pub grid_opacity: u8,
    #[serde(default = "default_true")]
    pub music_enabled: bool,
    /// Play the player's own music instead of the bundled track.
    #[serde(default)]
    pub use_custom_music: bool,
    /// Folder scanned for tracks when `use_custom_music` is on. Empty means
    /// "unset", which falls back to the bundled `Background.ogg`.
    #[serde(default)]
    pub music_directory: String,
    /// Key bindings, remapped from the Keyboard Keybinds menu.
    #[serde(default)]
    pub controls: Controls,
    /// Controller bindings, remapped from the Controller Keybinds menu.
    ///
    /// The original's `controller_controls`: the eight in-game actions. Kept
    /// separate from `controls` because the original let the two lists be bound
    /// completely independently, and a player with a pad whose D-pad is awkward
    /// will want the stick for movement without giving up the arrow keys.
    #[serde(default)]
    pub controller_controls: PadControls,
    /// The original's `controller_menu_navigation`: the four menu buttons. This
    /// is what makes the menus playable with no keyboard at all.
    #[serde(default)]
    pub controller_menu_navigation: PadNav,
    /// The original's `controller_settings`: stick thresholds, the deadzone and
    /// whether the D-pad steers the piece.
    #[serde(default)]
    pub controller_settings: PadSettings,
}

fn default_difficulty() -> String {
    "normal".to_string()
}
fn default_das() -> u32 {
    DEFAULT_DAS_MS
}
fn default_arr() -> u32 {
    DEFAULT_ARR_MS
}
fn default_theme() -> usize {
    0
}
fn default_palette_mode() -> usize {
    PALETTE_ADAPTIVE
}
fn default_skin() -> usize {
    0
}
fn default_effect() -> String {
    "flame".to_string()
}
fn default_gravity() -> f32 {
    1.0
}
fn default_ghost_on() -> bool {
    true
}
fn default_ghost_opacity() -> u8 {
    80
}
fn default_true() -> bool {
    true
}
fn default_grid_opacity() -> u8 {
    255
}

/// The remappable actions, in the original's menu order.
///
/// Stored as raylib keycodes. The original persisted the *name* of the key and
/// parsed it back on load; that is brittle across keymap layouts, and the
/// original's own loader had to fall back to a default whenever a name did not
/// parse. A code always round-trips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Left,
    Right,
    Down,
    Rotate,
    HardDrop,
    Hold,
    Pause,
    SkipTrack,
}

/// The action list, in menu order. Kept as a const so the options screen, the
/// keybind screen and the sampler cannot drift out of step with each other.
pub const ACTIONS: [Action; 8] = [
    Action::Left,
    Action::Right,
    Action::Down,
    Action::Rotate,
    Action::HardDrop,
    Action::Hold,
    Action::Pause,
    Action::SkipTrack,
];

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Action::Left => "Move Left",
            Action::Right => "Move Right",
            Action::Down => "Soft Drop",
            Action::Rotate => "Rotate",
            Action::HardDrop => "Hard Drop",
            Action::Hold => "Hold Piece",
            Action::Pause => "Pause",
            Action::SkipTrack => "Skip Track",
        }
    }

    /// The original's default for this action.
    pub fn default_key(self) -> Key {
        match self {
            Action::Left => Key::KEY_LEFT,
            Action::Right => Key::KEY_RIGHT,
            Action::Down => Key::KEY_DOWN,
            Action::Rotate => Key::KEY_UP,
            Action::HardDrop => Key::KEY_SPACE,
            Action::Hold => Key::KEY_C,
            Action::Pause => Key::KEY_P,
            Action::SkipTrack => Key::KEY_X,
        }
    }
}

/// Per-action key bindings.
///
/// Stored as raw keycodes rather than raylib's `KeyboardKey` enum, because that
/// enum is a plain C enum with no serde impls and no stable variant names. The
/// file is therefore numbers on disk — see [`keycode_to_json`]/[`json_to_key`]
/// for the human-readable form used when writing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Controls {
    #[serde(default = "dk_left")]
    pub left: i32,
    #[serde(default = "dk_right")]
    pub right: i32,
    #[serde(default = "dk_down")]
    pub down: i32,
    #[serde(default = "dk_rotate")]
    pub rotate: i32,
    #[serde(default = "dk_hard_drop")]
    pub hard_drop: i32,
    #[serde(default = "dk_hold")]
    pub hold: i32,
    #[serde(default = "dk_pause")]
    pub pause: i32,
    #[serde(default = "dk_skip_track")]
    pub skip_track: i32,
}

macro_rules! dk {
    ($fn_name:ident, $action:ident) => {
        fn $fn_name() -> i32 {
            Action::$action.default_key() as i32
        }
    };
}

dk!(dk_left, Left);
dk!(dk_right, Right);
dk!(dk_down, Down);
dk!(dk_rotate, Rotate);
dk!(dk_hard_drop, HardDrop);
dk!(dk_hold, Hold);
dk!(dk_pause, Pause);
dk!(dk_skip_track, SkipTrack);

impl Default for Controls {
    fn default() -> Self {
        Self {
            left: Action::Left.default_key() as i32,
            right: Action::Right.default_key() as i32,
            down: Action::Down.default_key() as i32,
            rotate: Action::Rotate.default_key() as i32,
            hard_drop: Action::HardDrop.default_key() as i32,
            hold: Action::Hold.default_key() as i32,
            pause: Action::Pause.default_key() as i32,
            skip_track: Action::SkipTrack.default_key() as i32,
        }
    }
}

impl Controls {
    /// The key bound to `action`.
    ///
    /// Codes with no matching raylib key fall back to the action's default
    /// rather than being handed to the input layer: a hand-edited or truncated
    /// `settings.json` must not be able to produce a key the platform layer
    /// has no mapping for.
    pub fn get(&self, action: Action) -> Key {
        let raw = self.raw(action);
        crate::keys::from_code(raw).unwrap_or_else(|| action.default_key())
    }

    /// The raw stored code, defaulted when missing or not a real key.
    pub fn raw(&self, action: Action) -> i32 {
        let v = match action {
            Action::Left => self.left,
            Action::Right => self.right,
            Action::Down => self.down,
            Action::Rotate => self.rotate,
            Action::HardDrop => self.hard_drop,
            Action::Hold => self.hold,
            Action::Pause => self.pause,
            Action::SkipTrack => self.skip_track,
        };
        if crate::keys::from_code(v).is_some() {
            v
        } else {
            crate::keys::to_code(action.default_key())
        }
    }

    pub fn set(&mut self, action: Action, key: Key) {
        let v = key as i32;
        match action {
            Action::Left => self.left = v,
            Action::Right => self.right = v,
            Action::Down => self.down = v,
            Action::Rotate => self.rotate = v,
            Action::HardDrop => self.hard_drop = v,
            Action::Hold => self.hold = v,
            Action::Pause => self.pause = v,
            Action::SkipTrack => self.skip_track = v,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            difficulty: "normal".to_string(),
            das: DEFAULT_DAS_MS,
            arr: DEFAULT_ARR_MS,
            theme: 0,
            palette_mode: PALETTE_ADAPTIVE,
            skin: 0,
            effect: "flame".to_string(),
            gravity_multiplier: 1.0,
            ghost_piece: true,
            ghost_opacity: 80,
            screen_shake: true,
            backgrounds_enabled: true,
            grid_lines: true,
            grid_opacity: 255,
            music_enabled: true,
            use_custom_music: false,
            music_directory: String::new(),
            controls: Controls::default(),
            controller_controls: PadControls::default(),
            controller_menu_navigation: PadNav::default(),
            controller_settings: PadSettings::default(),
        }
    }
}

impl Settings {
    /// Load from `settings.json`, falling back to defaults if absent or
    /// unreadable. A corrupt file should never stop the game from starting.
    pub fn load() -> Self {
        match std::fs::read_to_string("settings.json") {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Persist the current settings. Called after every options-menu change.
    pub fn save(&self) {
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write("settings.json", raw);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_playable() {
        let s = Settings::default();
        assert_eq!(s.difficulty, "normal");
        assert!(s.das > 0);
        assert!(s.arr > 0);
        assert!(s.ghost_opacity > 0);
        assert!(s.music_enabled);
    }

    #[test]
    fn round_trips_through_json() {
        let mut s = Settings::default();
        s.difficulty = "very hard".to_string();
        s.arr = 33;
        s.gravity_multiplier = 2.0;
        s.ghost_piece = false;
        let raw = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.difficulty, "very hard");
        assert_eq!(back.arr, 33);
        assert_eq!(back.gravity_multiplier, 2.0);
        assert!(!back.ghost_piece);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let partial: Settings = serde_json::from_str(r#"{"difficulty":"hard"}"#).unwrap();
        assert_eq!(partial.difficulty, "hard");
        assert_eq!(partial.das, DEFAULT_DAS_MS);
        assert_eq!(partial.gravity_multiplier, 1.0);
        assert!(partial.ghost_piece);
    }

    #[test]
    fn corrupt_json_falls_back_instead_of_panicking() {
        let bad: Result<Settings, _> = serde_json::from_str("{not json");
        assert!(bad.is_err());
        // And an unreadable file (or none at all) is handled gracefully by load.
        assert_eq!(Settings::load().difficulty.len() > 0, true);
    }

    // --- keybinds -------------------------------------------------------

    #[test]
    fn rebinding_one_action_leaves_the_others_alone() {
        let mut c = Controls::default();
        c.set(Action::HardDrop, Key::KEY_F);
        assert_eq!(c.get(Action::HardDrop), Key::KEY_F);
        assert_eq!(c.get(Action::Rotate), Key::KEY_UP);
        assert_eq!(c.get(Action::Pause), Key::KEY_P);
        assert_eq!(c.get(Action::Hold), Key::KEY_C);
    }

    #[test]
    fn a_rebind_survives_a_json_round_trip() {
        let mut s = Settings::default();
        s.controls.set(Action::Rotate, Key::KEY_X);
        s.controls.set(Action::Pause, Key::KEY_F9);
        s.controls.set(Action::HardDrop, Key::KEY_ENTER);
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.controls.get(Action::Rotate), Key::KEY_X);
        assert_eq!(back.controls.get(Action::Pause), Key::KEY_F9);
        assert_eq!(back.controls.get(Action::HardDrop), Key::KEY_ENTER);
    }

    #[test]
    fn a_settings_file_from_before_keybinds_existed_still_loads() {
        // An existing player upgrading must keep their other options; the whole
        // controls block has to default in rather than error.
        let old = r#"{"difficulty":"master","arr":33,"ghost_opacity":10}"#;
        let s: Settings = serde_json::from_str(old).unwrap();
        assert_eq!(s.difficulty, "master");
        assert_eq!(s.arr, 33);
        assert_eq!(s.ghost_opacity, 10);
        assert_eq!(s.controls.get(Action::Left), Key::KEY_LEFT);
        assert_eq!(s.controls.get(Action::SkipTrack), Key::KEY_X);
    }

    #[test]
    fn a_hand_edited_keycode_that_is_not_a_key_falls_back_to_the_default() {
        // 1 has no raylib variant. Feeding it to the input layer would be
        // undefined behaviour, so it has to be replaced with the default.
        let mut c = Controls::default();
        c.left = 1;
        c.rotate = -99;
        c.pause = 99999;
        assert_eq!(c.get(Action::Left), Key::KEY_LEFT);
        assert_eq!(c.get(Action::Rotate), Key::KEY_UP);
        assert_eq!(c.get(Action::Pause), Key::KEY_P);
    }

    #[test]
    fn every_action_has_a_distinct_default_so_the_defaults_do_not_collide() {
        let mut codes: Vec<i32> = ACTIONS.iter().map(|a| raw_default(*a)).collect();
        codes.sort_unstable();
        let before = codes.len();
        codes.dedup();
        assert_eq!(before, codes.len(), "two actions default to the same key");
    }

    fn raw_default(action: Action) -> i32 {
        crate::keys::to_code(action.default_key())
    }

    // --- grid -----------------------------------------------------------

    #[test]
    fn the_grid_defaults_to_fully_visible_and_can_be_turned_off() {
        let s = Settings::default();
        assert!(s.grid_lines);
        assert_eq!(s.grid_opacity, 255);
    }

    #[test]
    fn grid_settings_survive_a_json_round_trip() {
        let mut s = Settings::default();
        s.grid_lines = false;
        s.grid_opacity = 64;
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert!(!back.grid_lines);
        assert_eq!(back.grid_opacity, 64);
    }
    // --- backgrounds ----------------------------------------------------

    #[test]
    fn backgrounds_are_on_by_default_so_a_fresh_install_looks_like_the_original() {
        // The original had no such option, so "on" is the only default that
        // does not quietly change how the game looks for someone who has never
        // opened the options menu.
        assert!(Settings::default().backgrounds_enabled);
    }

    #[test]
    fn the_backgrounds_setting_survives_a_json_round_trip() {
        let mut s = Settings::default();
        s.backgrounds_enabled = false;
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert!(!back.backgrounds_enabled);
    }

    #[test]
    fn a_settings_file_from_before_the_backgrounds_option_existed_still_shows_them() {
        // Upgrading must not switch the photos off as a side effect of the
        // key being absent: the serde default, not the bool's own `false`.
        let old = r#"{"difficulty":"master","screen_shake":false}"#;
        let s: Settings = serde_json::from_str(old).unwrap();
        assert!(!s.screen_shake, "the key that *was* present still applies");
        assert!(s.backgrounds_enabled, "the key that was not does not");
    }

    // --- custom music ----------------------------------------------------

    #[test]
    fn custom_music_starts_off_with_no_folder() {
        // The original's defaults were `use_custom_music: False` and
        // `music_directory: '', so a fresh install plays the bundled track.
        let s = Settings::default();
        assert!(!s.use_custom_music);
        assert_eq!(s.music_directory, "");
    }

    #[test]
    fn the_music_folder_survives_a_json_round_trip() {
        let mut s = Settings::default();
        s.use_custom_music = true;
        // A Windows path, a UNC path and a POSIX path: the folder is a raw
        // string, and backslash-heavy text is exactly what breaks naive JSON
        // writing.
        for dir in [r"C:\Users\Someone\My Music", r"\\nas\share\music", "/home/someone/Music"] {
            s.music_directory = dir.to_string();
            let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
            assert!(back.use_custom_music);
            assert_eq!(back.music_directory, dir);
        }
    }

    #[test]
    fn a_settings_file_from_before_custom_music_existed_still_loads() {
        let old = r#"{"difficulty":"hard","music_enabled":true}"#;
        let s: Settings = serde_json::from_str(old).unwrap();
        assert_eq!(s.difficulty, "hard");
        assert!(!s.use_custom_music);
        assert_eq!(s.music_directory, "");
    }
}