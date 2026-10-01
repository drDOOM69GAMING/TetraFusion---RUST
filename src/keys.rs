//! Keycode <-> name mapping for the rebinding menu.
//!
//! raylib's `KeyboardKey` is a bindgen-generated C enum: it has no serde impls,
//! no `TryFrom`, and **no way to turn an arbitrary integer back into a variant**.
//! Its discriminants are also not contiguous (they are ASCII codes for the
//! printable keys and raylib-synthesised values above 256 for the rest), so a
//! `0..=348` range check would happily accept `1` - a value with no variant at
//! all - and transmuting to it is undefined behaviour.
//!
//! So the mapping is written out as an explicit table. That table is also the
//! display name for each binding, which is why the two concerns live together:
//! there is exactly one list of "keys this game knows about", and it is checked
//! against the real raylib enum by a test rather than trusted.

use raylib::consts::KeyboardKey as Key;

/// `(variant, display name, keycode)` for every key the game will bind.
///
/// The codes are asserted against raylib's own enum in
/// `every_table_code_matches_the_raylib_enum`, so this list cannot silently
/// drift away from the version of raylib it is compiled against.
const TABLE: &[(Key, &str, i32)] = &[
    (Key::KEY_NULL, "NONE", 0),
    (Key::KEY_APOSTROPHE, "APOSTROPHE", 39),
    (Key::KEY_COMMA, "COMMA", 44),
    (Key::KEY_MINUS, "MINUS", 45),
    (Key::KEY_PERIOD, "PERIOD", 46),
    (Key::KEY_SLASH, "SLASH", 47),
    (Key::KEY_ZERO, "0", 48),
    (Key::KEY_ONE, "1", 49),
    (Key::KEY_TWO, "2", 50),
    (Key::KEY_THREE, "3", 51),
    (Key::KEY_FOUR, "4", 52),
    (Key::KEY_FIVE, "5", 53),
    (Key::KEY_SIX, "6", 54),
    (Key::KEY_SEVEN, "7", 55),
    (Key::KEY_EIGHT, "8", 56),
    (Key::KEY_NINE, "9", 57),
    (Key::KEY_SEMICOLON, "SEMICOLON", 59),
    (Key::KEY_EQUAL, "EQUAL", 61),
    (Key::KEY_A, "A", 65),
    (Key::KEY_B, "B", 66),
    (Key::KEY_C, "C", 67),
    (Key::KEY_D, "D", 68),
    (Key::KEY_E, "E", 69),
    (Key::KEY_F, "F", 70),
    (Key::KEY_G, "G", 71),
    (Key::KEY_H, "H", 72),
    (Key::KEY_I, "I", 73),
    (Key::KEY_J, "J", 74),
    (Key::KEY_K, "K", 75),
    (Key::KEY_L, "L", 76),
    (Key::KEY_M, "M", 77),
    (Key::KEY_N, "N", 78),
    (Key::KEY_O, "O", 79),
    (Key::KEY_P, "P", 80),
    (Key::KEY_Q, "Q", 81),
    (Key::KEY_R, "R", 82),
    (Key::KEY_S, "S", 83),
    (Key::KEY_T, "T", 84),
    (Key::KEY_U, "U", 85),
    (Key::KEY_V, "V", 86),
    (Key::KEY_W, "W", 87),
    (Key::KEY_X, "X", 88),
    (Key::KEY_Y, "Y", 89),
    (Key::KEY_Z, "Z", 90),
    (Key::KEY_LEFT_BRACKET, "LBRACKET", 91),
    (Key::KEY_BACKSLASH, "BACKSLASH", 92),
    (Key::KEY_RIGHT_BRACKET, "RBRACKET", 93),
    (Key::KEY_GRAVE, "GRAVE", 96),
    (Key::KEY_SPACE, "SPACE", 32),
    (Key::KEY_ESCAPE, "ESCAPE", 256),
    (Key::KEY_ENTER, "ENTER", 257),
    (Key::KEY_TAB, "TAB", 258),
    (Key::KEY_BACKSPACE, "BACKSPACE", 259),
    (Key::KEY_INSERT, "INSERT", 260),
    (Key::KEY_DELETE, "DELETE", 261),
    (Key::KEY_RIGHT, "RIGHT", 262),
    (Key::KEY_LEFT, "LEFT", 263),
    (Key::KEY_DOWN, "DOWN", 264),
    (Key::KEY_UP, "UP", 265),
    (Key::KEY_PAGE_UP, "PAGE UP", 266),
    (Key::KEY_PAGE_DOWN, "PAGE DOWN", 267),
    (Key::KEY_HOME, "HOME", 268),
    (Key::KEY_END, "END", 269),
    (Key::KEY_CAPS_LOCK, "CAPS LOCK", 280),
    (Key::KEY_SCROLL_LOCK, "SCROLL LOCK", 281),
    (Key::KEY_NUM_LOCK, "NUM LOCK", 282),
    (Key::KEY_PRINT_SCREEN, "PRINT SCREEN", 283),
    (Key::KEY_PAUSE, "PAUSE", 284),
    (Key::KEY_F1, "F1", 290),
    (Key::KEY_F2, "F2", 291),
    (Key::KEY_F3, "F3", 292),
    (Key::KEY_F4, "F4", 293),
    (Key::KEY_F5, "F5", 294),
    (Key::KEY_F6, "F6", 295),
    (Key::KEY_F7, "F7", 296),
    (Key::KEY_F8, "F8", 297),
    (Key::KEY_F9, "F9", 298),
    (Key::KEY_F10, "F10", 299),
    (Key::KEY_F11, "F11", 300),
    (Key::KEY_F12, "F12", 301),
    (Key::KEY_LEFT_SHIFT, "LSHIFT", 340),
    (Key::KEY_LEFT_CONTROL, "LCTRL", 341),
    (Key::KEY_LEFT_ALT, "LALT", 342),
    (Key::KEY_LEFT_SUPER, "LSUPER", 343),
    (Key::KEY_RIGHT_SHIFT, "RSHIFT", 344),
    (Key::KEY_RIGHT_CONTROL, "RCTRL", 345),
    (Key::KEY_RIGHT_ALT, "RALT", 346),
    (Key::KEY_RIGHT_SUPER, "RSUPER", 347),
    (Key::KEY_KB_MENU, "MENU", 348),
    (Key::KEY_KP_0, "KP 0", 320),
    (Key::KEY_KP_1, "KP 1", 321),
    (Key::KEY_KP_2, "KP 2", 322),
    (Key::KEY_KP_3, "KP 3", 323),
    (Key::KEY_KP_4, "KP 4", 324),
    (Key::KEY_KP_5, "KP 5", 325),
    (Key::KEY_KP_6, "KP 6", 326),
    (Key::KEY_KP_7, "KP 7", 327),
    (Key::KEY_KP_8, "KP 8", 328),
    (Key::KEY_KP_9, "KP 9", 329),
    (Key::KEY_KP_DECIMAL, "KP .", 330),
    (Key::KEY_KP_DIVIDE, "KP /", 331),
    (Key::KEY_KP_MULTIPLY, "KP *", 332),
    (Key::KEY_KP_SUBTRACT, "KP -", 333),
    (Key::KEY_KP_ADD, "KP +", 334),
    (Key::KEY_KP_ENTER, "KP ENTER", 335),
    (Key::KEY_KP_EQUAL, "KP =", 336),
    (Key::KEY_BACK, "BACK", 4),
    (Key::KEY_MENU, "APPS MENU", 5),
    (Key::KEY_VOLUME_UP, "VOL UP", 24),
    (Key::KEY_VOLUME_DOWN, "VOL DOWN", 25),
];

/// What a raw key press means while the rebinding screen is waiting for one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pressed {
    /// No key was pressed this frame.
    Nothing,
    /// Bind the action to this key.
    Bind(Key),
    /// Abandon the rebind, leaving the existing binding alone.
    Cancel,
}

/// Classify a key press during a rebind.
///
/// While waiting for a key the screen deliberately swallows *every* key press,
/// because the player may legitimately want to bind a key that is also a
/// navigation or action key. The one exception is Escape: the prompt promises
/// that Escape cancels, and without this it would quietly bind Escape to
/// whatever action was being edited - taking away the player's way out.
pub fn classify_capture(pressed: Option<Key>) -> Pressed {
    match pressed {
        None => Pressed::Nothing,
        Some(Key::KEY_ESCAPE) => Pressed::Cancel,
        Some(k) => Pressed::Bind(k),
    }
}

/// The key for a stored code, or `None` if no raylib variant has that value.
///
/// This is the only sound way to go back from a stored integer to a
/// `KeyboardKey`; see the module docs for why.
pub fn from_code(code: i32) -> Option<Key> {
    TABLE
        .iter()
        .find(|(_, _, c)| *c == code)
        .map(|(k, _, _)| *k)
}

/// The stored code for a key. Always succeeds: raylib can only ever hand us a
/// variant that is in the table, because the table is exhaustive and checked.
pub fn to_code(key: Key) -> i32 {
    TABLE
        .iter()
        .find(|(k, _, _)| *k == key)
        .map(|(_, _, c)| *c)
        .unwrap_or(key as i32)
}

/// A short name for a key, for the keybind screen. Falls back to the raw number
/// so an unexpected binding still reads as something the player can recognise
/// as "not a key I know".
pub fn name(key: Key) -> String {
    TABLE
        .iter()
        .find(|(k, _, _)| *k == key)
        .map(|(_, n, _)| (*n).to_string())
        .unwrap_or_else(|| format!("#{}", key as i32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_code_matches_the_raylib_enum() {
        // The whole point of the table is to be a checked stand-in for a
        // conversion raylib does not provide, so verify it against raylib.
        for (key, name, code) in TABLE {
            assert_eq!(
                *key as i32, *code,
                "table code for {name} disagrees with raylib's discriminant"
            );
        }
    }

    #[test]
    fn no_two_keys_share_a_code() {
        let mut codes: Vec<i32> = TABLE.iter().map(|(_, _, c)| *c).collect();
        codes.sort_unstable();
        let before = codes.len();
        codes.dedup();
        assert_eq!(before, codes.len(), "duplicate keycode in the table");
    }

    #[test]
    fn no_two_keys_share_a_name() {
        let mut names: Vec<&str> = TABLE.iter().map(|(_, n, _)| *n).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "duplicate display name in the table");
    }

    #[test]
    fn every_default_binding_round_trips_through_the_table() {
        use crate::settings::ACTIONS;
        for action in ACTIONS {
            let key = action.default_key();
            let code = to_code(key);
            assert_eq!(
                from_code(code),
                Some(key),
                "{} default does not survive a save/load round trip",
                action.label()
            );
        }
    }

    #[test]
    fn codes_with_no_raylib_variant_are_rejected() {
        // The gap between the ASCII printable keys: 1, 2, 3 have no variant,
        // and neither does anything past KEY_KB_MENU.
        for bogus in [1, 2, 3, 30, 46 + 1000, 349, 9999, -5, i32::MIN, i32::MAX] {
            assert_eq!(
                from_code(bogus),
                None,
                "code {bogus} has no raylib variant but was accepted"
            );
        }
    }

    #[test]
    fn the_printable_and_navigation_keys_are_all_bindable() {
        // Spot-check the keys the defaults use plus the alternates the input
        // layer still accepts, so a refactor cannot quietly drop one.
        for key in [
            Key::KEY_LEFT,
            Key::KEY_RIGHT,
            Key::KEY_DOWN,
            Key::KEY_UP,
            Key::KEY_SPACE,
            Key::KEY_ENTER,
            Key::KEY_ESCAPE,
            Key::KEY_C,
            Key::KEY_P,
            Key::KEY_X,
            Key::KEY_A,
            Key::KEY_D,
            Key::KEY_S,
            Key::KEY_W,
            Key::KEY_Z,
            Key::KEY_LEFT_SHIFT,
            Key::KEY_LEFT_CONTROL,
            Key::KEY_F11,
        ] {
            assert_eq!(
                from_code(to_code(key)),
                Some(key),
                "{:?} is missing from the key table",
                name(key)
            );
        }
    }

    #[test]
    fn names_are_short_enough_to_fit_the_keybind_row() {
        // The row is centred text at 22px; anything long would overflow the
        // 450px column and collide with the sub-window.
        for (_, name, _) in TABLE {
            assert!(
                name.len() <= 12,
                "display name {name:?} is too wide for the keybind row"
            );
        }
    }

    #[test]
    fn escape_cancels_a_capture_instead_of_binding_itself() {
        // Regression guard: the capture swallows every key, so without the
        // Escape special case a rebind of Pause would bind Escape to Pause and
        // remove the player's way out of a run.
        assert_eq!(classify_capture(Some(Key::KEY_ESCAPE)), Pressed::Cancel);
    }

    #[test]
    fn any_other_key_binds_including_the_navigation_keys() {
        // Binding is meant to be unconstrained, so the keys that navigate and
        // confirm must still be bindable.
        for key in [
            Key::KEY_ENTER,
            Key::KEY_SPACE,
            Key::KEY_UP,
            Key::KEY_DOWN,
            Key::KEY_LEFT,
            Key::KEY_RIGHT,
            Key::KEY_TAB,
        ] {
            assert_eq!(classify_capture(Some(key)), Pressed::Bind(key));
        }
    }

    #[test]
    fn no_key_press_does_nothing() {
        assert_eq!(classify_capture(None), Pressed::Nothing);
    }
}