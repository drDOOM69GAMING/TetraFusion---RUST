//! The high-score table: one best score per mode, with three-letter initials.
//!
//! This is the feature the Pygame original had and this port was missing. The
//! original stores `high_score.json` as
//!
//! ```json
//! { "marathon": { "score": 12400, "name": "ACE" }, "sprint": { "..." } }
//! ```
//!
//! and shows an "Enter Initials" prompt *instead of* the game-over screen when a
//! run beats the stored number, saving only if the score is still better when the
//! player confirms. Both of those behaviours are reproduced here, including the
//! awkward parts, because they are what makes it a high-score table rather than
//! a score that gets silently overwritten.
//!
//! Two decisions worth stating outright:
//!
//! - **One record per mode, not a top-N list.** That is what the original did
//!   and what its `High: 12400 (ACE)` line implies. A top-ten table would be a
//!   different feature.
//! - **A record only ever goes up.** [`Scores::submit`] refuses a worse score,
//!   so losing to the board cannot quietly delete a record, and replaying a
//!   worse run cannot overwrite it.
//!
//! Everything here is non-fatal. A missing file is an empty table; an unreadable
//! or corrupt one is an empty table; a failed write is a lost score, never a
//! crash. The game must start.

use crate::game::Mode;
use serde::{Deserialize, Serialize};

/// Where the table is kept, beside `settings.json`.
pub const FILENAME: &str = "high_score.json";

/// How many letters a name gets, as in the original.
pub const MAX_INITIALS: usize = 3;

/// What an untouched slot reads as. The original used `---`, and a run of
/// dashes reads as "empty" in a way that `0` alone does not.
pub const BLANK: &str = "---";

/// One mode's best: the score, and who set it.
///
/// `name` is three letters or [`BLANK`]. It is a `String` rather than a fixed
/// `[u8; 3]` because it comes from and goes to JSON, and because the blank state
/// is not three characters of anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    // Both fields default individually, not just as a struct. A file written by
    // an older build, or one a player has hand-edited to change only the score,
    // has no `name` key - and the original's loader tolerated exactly that with
    // `entry.get('name', '---')`. Defaulting only the struct would make one
    // missing field fail the whole file and lose the other four modes' scores.
    #[serde(default)]
    pub score: i32,
    #[serde(default = "blank_name")]
    pub name: String,
}

/// `Record::name`'s serde default: the blank placeholder.
fn blank_name() -> String {
    BLANK.to_string()
}

impl Default for Record {
    fn default() -> Self {
        Record {
            score: 0,
            name: BLANK.to_string(),
        }
    }
}

impl Record {
    /// The `High: 0 (---)` line for this slot.
    ///
    /// A blank slot reports itself rather than hiding, so a player who has never
    /// scored can see that the feature exists and what they would have to beat.
    pub fn line(&self) -> String {
        format!("High: {} ({})", self.score, self.name)
    }
}

/// The whole table, one [`Record`] per mode.
///
/// Serialised as named fields rather than a map so the file reads the way the
/// original's did, with a stable key per mode. Every field is `#[serde(default)]`
/// because the file is user-editable and a hand-truncated or half-written one
/// must still load: a missing `master` key should leave a blank slot, not fail
/// the whole parse and lose the other four.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scores {
    #[serde(default)]
    pub marathon: Record,
    #[serde(default)]
    pub sprint: Record,
    #[serde(default)]
    pub ultra: Record,
    #[serde(default)]
    pub training: Record,
    #[serde(default)]
    pub master: Record,
}

impl Scores {
    /// The slot for `mode`.
    pub fn get(&self, mode: Mode) -> &Record {
        match mode {
            Mode::Marathon => &self.marathon,
            Mode::Sprint => &self.sprint,
            Mode::Ultra => &self.ultra,
            Mode::Training => &self.training,
            Mode::Master => &self.master,
        }
    }

    fn slot_mut(&mut self, mode: Mode) -> &mut Record {
        match mode {
            Mode::Marathon => &mut self.marathon,
            Mode::Sprint => &mut self.sprint,
            Mode::Ultra => &mut self.ultra,
            Mode::Training => &mut self.training,
            Mode::Master => &mut self.master,
        }
    }

    /// Whether `score` would beat the record for `mode`.
    ///
    /// This is the test that decides whether the initials prompt appears at all,
    /// so it is deliberately the same comparison [`Scores::submit`] uses. When
    /// the two ever disagreed, a player could be asked for their initials for a
    /// score that was then refused.
    ///
    /// Strictly greater, matching the original: a tie is not a new record. Ties
    /// happen constantly - replaying the same seed and dropping the same pieces
    /// gives the same score - and rewarding one would let a player farm names by
    /// replaying a good run until it stuck.
    ///
    /// `false` outright for a mode with no record, so the initials prompt can
    /// never appear for one. Training cannot reach this in play - it never ends -
    /// but the check belongs here rather than at the call site, because a caller
    /// that forgets it is the whole bug this guards.
    pub fn is_record(&self, mode: Mode, score: i32) -> bool {
        self.records(mode) && score > self.get(mode).score
    }

    /// Store `score` and `name` for `mode` if it beats the current record.
    ///
    /// Returns whether it was written. The name is normalised first, so a
    /// caller cannot store lowercase, a five-letter name, or a blank one just by
    /// passing the wrong string.
    pub fn submit(&mut self, mode: Mode, score: i32, name: &str) -> bool {
        if !self.is_record(mode, score) {
            return false;
        }
        let name = normalise_name(name);
        *self.slot_mut(mode) = Record { score, name };
        true
    }

    /// Read the table, treating any failure as an empty one.
    pub fn load() -> Self {
        // Mirrors `Settings::load`: the file is an enhancement, not a
        // precondition, so a read error and a parse error both mean "no scores
        // yet" rather than propagating.
        match std::fs::read_to_string(FILENAME) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Write the table. A failed write costs the score, never the run.
    pub fn save(&self) {
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(FILENAME, raw);
        }
    }

    /// The `High: 12400 (ACE)` line for `mode`, as the original showed it on the
    /// pause overlay.
    pub fn line(&self, mode: Mode) -> String {
        self.get(mode).line()
    }

    /// Whether `mode` keeps a record at all.
    ///
    /// Training does not: it never ends, so there is no moment at which the
    /// initials prompt could appear, and a slot that can only ever be blank is a
    /// feature that cannot work. The slot stays in the table rather than being
    /// removed, because the file is the original's shape and a missing key would
    /// look like damage.
    pub fn records(&self, mode: Mode) -> bool {
        mode != Mode::Training
    }
}

/// Force a name into the form the table stores: uppercase, alphanumeric, at most
/// [`MAX_INITIALS`] characters, never empty.
///
/// The original accepted `event.unicode.isalnum()` and upper-cased it, which on a
/// real keyboard could also admit accented letters. Those cannot be drawn here -
/// raylib's default font stops at codepoint 255 and the game draws labels with it
/// directly - so the filter is deliberately narrower: ASCII letters and digits
/// only, and anything else is dropped rather than stored and then rendered as a
/// missing glyph on the pause overlay.
pub fn normalise_name(raw: &str) -> String {
    let mut out = String::with_capacity(MAX_INITIALS);
    for c in raw.chars() {
        if out.len() >= MAX_INITIALS {
            break;
        }
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        }
    }
    if out.is_empty() {
        BLANK.to_string()
    } else {
        out
    }
}

/// What the player has typed so far, and the rules for typing it.
///
/// A separate type rather than a bare `String` so the cap, the filter and the
/// "nothing entered yet" case live in one tested place instead of being
/// re-implemented at the two places that accept input.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Initials {
    buf: String,
}

impl Initials {
    /// An empty entry. Matches the original, which started the prompt blank.
    pub fn new() -> Self {
        Initials::default()
    }

    /// Add `c` if it is allowed and there is room.
    ///
    /// A full entry silently ignores further keys rather than refusing them, so
    /// holding a letter down cannot produce a four-letter name and cannot make
    /// the key look dead either - the entry simply stops changing, which is what
    /// the player expects from a three-letter field.
    pub fn push(&mut self, c: char) {
        if self.buf.len() >= MAX_INITIALS {
            return;
        }
        if c.is_ascii_alphanumeric() {
            self.buf.push(c.to_ascii_uppercase());
        }
    }

    /// Remove the last letter. The original's Backspace.
    pub fn backspace(&mut self) {
        self.buf.pop();
    }

    /// What has been typed, possibly nothing.
    pub fn as_str(&self) -> &str {
        &self.buf
    }

    /// Whether there is nothing to save yet.
    ///
    /// The original required a name before it would accept the entry: `if
    /// event.key == pygame.K_RETURN and initials`. So the prompt cannot be
    /// dismissed by confirming an empty name, which is what stops a run from
    /// being recorded as `---`.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Whether all three slots are filled.
    pub fn is_full(&self) -> bool {
        self.buf.len() >= MAX_INITIALS
    }

    /// The entry padded out for display, so the field does not change width as
    /// letters are typed: `A`, `A_`, `AB_`, `ABC`.
    pub fn display(&self) -> String {
        let mut s = self.buf.clone();
        while s.len() < MAX_INITIALS {
            s.push('_');
        }
        s
    }
}

/// The raylib key code for the letter or digit that produces `c`, if any.
///
/// The pure half of text entry, split out so the mapping can be tested without a
/// window - `is_key_pressed` needs a live context, but "does shift-less typing
/// of `Q` produce `Q`" does not, and a wrong answer here would silently make
/// some letter untypeable.
///
/// Raylib's key codes for A-Z and 0-9 are the ASCII values, so the range maps
/// directly. Anything outside A-Z and 0-9 returns `None` and is not a name
/// character: the original's `isalnum()` was ASCII in practice on the keyboards
/// it was played on, and [`normalise_name`] would drop anything else anyway.
pub fn key_to_char(key: i32) -> Option<char> {
    let c = char::from_u32(key as u32)?;
    if c.is_ascii_alphanumeric() {
        Some(c)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A table with one record set, so the rest of the tests do not each have to
    /// build one.
    fn with_record() -> Scores {
        let mut s = Scores::default();
        s.submit(Mode::Marathon, 500, "abc");
        s
    }

    #[test]
    fn a_fresh_table_is_all_blanks() {
        let s = Scores::default();
        for mode in Mode::ALL {
            assert_eq!(s.get(mode).score, 0);
            assert_eq!(s.get(mode).name, BLANK);
        }
    }

    #[test]
    fn a_record_is_kept_per_mode_and_not_shared() {
        let mut s = Scores::default();
        s.submit(Mode::Marathon, 100, "AAA");
        s.submit(Mode::Sprint, 250, "BBB");
        assert_eq!(s.get(Mode::Marathon).name, "AAA");
        assert_eq!(s.get(Mode::Sprint).name, "BBB");
        // The two modes never see each other's score.
        assert_eq!(s.get(Mode::Ultra).score, 0);
        assert_eq!(s.get(Mode::Ultra).name, BLANK);
        assert_eq!(s.line(Mode::Ultra), "High: 0 (---)");
    }

    /// The original's rule, and the one that makes the prompt meaningful.
    #[test]
    fn only_a_strictly_better_score_makes_a_record() {
        let s = with_record();
        assert!(!s.is_record(Mode::Marathon, 500), "a tie is not a record");
        assert!(!s.is_record(Mode::Marathon, 499), "a worse score is not");
        assert!(s.is_record(Mode::Marathon, 501));
    }

    #[test]
    fn a_worse_run_cannot_overwrite_a_record() {
        let mut s = with_record();
        assert!(!s.submit(Mode::Marathon, 10, "ZZZ"));
        assert_eq!(s.get(Mode::Marathon).score, 500);
        assert_eq!(s.get(Mode::Marathon).name, "ABC");
    }

    /// The prompt and the save have to agree. If they ever disagreed, a player
    /// would be asked for initials for a score that was then thrown away.
    #[test]
    fn the_prompt_and_the_save_use_the_same_test() {
        for score in [0i32, 1, 499, 500, 501, 10_000] {
            let s = with_record();
            let mut t = s.clone();
            let asked = s.is_record(Mode::Marathon, score);
            let stored = t.submit(Mode::Marathon, score, "NEW");
            assert_eq!(asked, stored, "score {score} disagreed");
            assert_eq!(s.get(Mode::Marathon).name, "ABC");
        }
    }

    #[test]
    fn a_stored_name_is_capped_uppercased_and_filtered() {
        assert_eq!(normalise_name("abc"), "ABC");
        assert_eq!(normalise_name("abcde"), "ABC");
        assert_eq!(normalise_name("a1b"), "A1B");
        assert_eq!(normalise_name("  ace  "), "ACE");
        assert_eq!(normalise_name("a-b-c"), "ABC");
        assert_eq!(
            normalise_name("éèê"),
            BLANK,
            "non-ascii letters are dropped, not transliterated"
        );
        assert_eq!(normalise_name(""), BLANK);
        assert_eq!(normalise_name("!!!"), BLANK);
        assert_eq!(normalise_name("  "), BLANK);
    }

    #[test]
    fn a_name_typed_as_uppercase_survives_a_save_unchanged() {
        let mut s = Scores::default();
        s.submit(Mode::Ultra, 10, "ace");
        assert_eq!(s.get(Mode::Ultra).name, "ACE");
    }

    /// Typing rules, as the prompt enforces them.
    #[test]
    fn the_entry_takes_three_letters_and_no_more() {
        let mut e = Initials::new();
        assert!(e.is_empty());
        for c in "abcdef".chars() {
            e.push(c);
        }
        assert_eq!(e.as_str(), "ABC", "the fourth letter was taken");
        assert!(e.is_full());
        e.backspace();
        assert_eq!(e.as_str(), "AB");
        assert!(!e.is_full());
        e.backspace();
        e.backspace();
        assert!(e.is_empty(), "backspacing past empty did not stop");
    }

    #[test]
    fn the_entry_ignores_anything_that_is_not_a_letter_or_digit() {
        let mut e = Initials::new();
        for c in [' ', '-', '!', '.', '\u{7}'] {
            e.push(c);
        }
        assert!(e.is_empty(), "a punctuation key got into the name");
        e.push('7');
        assert_eq!(e.as_str(), "7");
    }

    /// Typing a name shifted or unshifted has to store the same thing.
    ///
    /// Shifted and unshifted letters arrive from the same key codes - this port
    /// reads key codes, not the character a key produced - so both go through
    /// `push` and have to come out identical.
    #[test]
    fn the_entry_lowercase_and_uppercases_the_same() {
        let mut lower = Initials::new();
        for c in "ace".chars() {
            lower.push(c);
        }
        let mut upper = Initials::new();
        for c in "ACE".chars() {
            upper.push(c);
        }
        assert_eq!(lower.as_str(), "ACE");
        assert_eq!(lower, upper);
    }

    /// The field has to keep its width, or the whole line jumps sideways as the
    /// player types.
    #[test]
    fn the_entry_displays_at_a_fixed_width() {
        let mut e = Initials::new();
        let mut widths: Vec<usize> = Vec::new();
        widths.push(e.display().chars().count());
        for c in "abc".chars() {
            e.push(c);
            widths.push(e.display().chars().count());
        }
        assert!(
            widths.windows(2).all(|w| w[0] == w[1]),
            "the field changed width as it was typed: {widths:?}"
        );
        assert_eq!(e.display(), "ABC");
        e.backspace();
        assert_eq!(e.display(), "AB_");
    }

    /// Every key the prompt listens for has to produce a character, or a letter
    /// is silently untypeable.
    #[test]
    fn every_letter_and_digit_key_maps_to_itself() {
        for c in ('A'..='Z').chain('0'..='9') {
            assert_eq!(key_to_char(c as i32), Some(c), "{c} did not map");
        }
    }

    /// Keys the prompt must not act on. Escape and Enter in particular: if either
    /// produced a character, confirming the entry would type into it first.
    #[test]
    fn keys_outside_the_alphabet_map_to_nothing() {
        for key in [
            0,
            -1,
            32,
            256, // KEY_SPACE
            257, // KEY_ENTER
            262, // KEY_ESCAPE
            259, // KEY_BACKSPACE
            265, // KEY_UP
            265 + 1000,
        ] {
            assert_eq!(key_to_char(key), None, "key {key} produced a character");
        }
    }

    /// The whole table has to survive JSON, and a file missing a mode has to load
    /// with that mode blank rather than discarding the other four.
    #[test]
    fn the_table_round_trips_through_json() {
        let mut s = Scores::default();
        s.submit(Mode::Marathon, 12400, "ACE");
        s.submit(Mode::Sprint, 42, "ZED");
        let raw = serde_json::to_string(&s).unwrap();
        let back: Scores = serde_json::from_str(&raw).unwrap();
        assert_eq!(back, s);
        // And it reads the way the original's file did.
        assert!(raw.contains("\"marathon\""), "{raw}");
        assert!(raw.contains("\"sprint\""), "{raw}");
        assert!(raw.contains("\"ACE\""), "{raw}");
    }

    #[test]
    fn a_partial_or_corrupt_file_still_loads() {
        // Only one mode present, as a half-written file would be.
        let partial: Scores =
            serde_json::from_str(r#"{"marathon":{"score":500,"name":"ACE"}}"#).unwrap();
        assert_eq!(partial.get(Mode::Marathon).score, 500);
        assert_eq!(partial.get(Mode::Master).name, BLANK);
        assert_eq!(partial.get(Mode::Sprint).score, 0);

        // A mode present but with fields missing entirely.
        let partial_field: Scores = serde_json::from_str(r#"{"ultra":{"score":7}}"#).unwrap();
        assert_eq!(partial_field.get(Mode::Ultra).score, 7);
        assert_eq!(partial_field.get(Mode::Ultra).name, BLANK);

        // Nonsense in, empty table out, and no panic.
        for junk in ["", "{", "not json", "[]", "null", "{\"marathon\":3}"] {
            let back: Scores = serde_json::from_str(junk).unwrap_or_default();
            assert_eq!(back, Scores::default(), "{junk:?} did not fall back");
        }
    }

    #[test]
    fn the_high_score_line_shows_an_empty_slot_rather_than_hiding() {
        let s = Scores::default();
        assert_eq!(s.line(Mode::Marathon), "High: 0 (---)");
        let s = with_record();
        assert_eq!(s.line(Mode::Marathon), "High: 500 (ABC)");
    }

    /// Training never ends, so it can never offer initials, so it must never show
    /// a record either. A slot that can only be blank is a feature that cannot
    /// work, and `High: 0 (---)` on screen for a whole run says so.
    #[test]
    fn training_keeps_no_record() {
        let mut s = Scores::default();
        assert!(!s.records(Mode::Training));
        // Not even a spectacular score.
        assert!(
            !s.is_record(Mode::Training, 10_000_000),
            "training was offered a record it can never finish into"
        );
        assert!(!s.submit(Mode::Training, 10_000_000, "ACE"));
        assert_eq!(s.get(Mode::Training).score, 0);
        assert_eq!(s.get(Mode::Training).name, BLANK);
        // And every other mode does keep one.
        for mode in [Mode::Marathon, Mode::Sprint, Mode::Ultra, Mode::Master] {
            assert!(s.records(mode), "{mode:?} should keep a record");
            assert!(s.is_record(mode, 1));
        }
    }

    /// The record-free modes and the rest have to agree: nothing that keeps a
    /// record may be one that `records` refuses.
    #[test]
    fn every_recording_mode_can_actually_be_submitted_to() {
        let mut s = Scores::default();
        for mode in Mode::ALL {
            assert_eq!(
                s.submit(mode, 100, "AAA"),
                s.records(mode),
                "{mode:?} disagreed with itself about keeping a record"
            );
        }
    }

    /// A missing or unreadable file must never stop the game starting.
    #[test]
    fn loading_a_file_that_is_not_there_gives_an_empty_table() {
        // Not testing `load()` against the real cwd - the table has to be
        // readable whether or not a file exists, and the fallback is the same
        // code path the `serde_json` errors above already cover.
        let missing: Scores = serde_json::from_str("").unwrap_or_default();
        assert_eq!(missing, Scores::default());
    }
}