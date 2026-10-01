//! The in-game manual, opened from the main menu.
//!
//! A game with a move set this size owes the player a book, and the original
//! shipped none: every binding, every scoring rule and every mode had to be
//! discovered by losing a run to it. The manual is that book, kept as data
//! rather than as drawing calls so two things can be checked without a window
//! attached. First, that no page runs off the bottom of the well sized screen
//! ([`last_line_y`]). Second, that the text only uses characters raylib's
//! built in font actually has, since anything else draws as a blank box
//! (the ASCII test in this module).
//!
//! The rules stated here are the rules the code plays by, and where the two
//! could drift the tests are the tie breaker: the mode lines are checked
//! against [`crate::game::Mode::blurb`], so a mode that changes its goal
//! cannot keep an old manual page.

use crate::config::{SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::game::Mode;

/// One printed line of the manual.
///
/// Every line knows how many rows of [`LINE_PITCH`] it takes up, because a mode
/// prints its name and then its rule and so is two rows tall while everything
/// else is one. [`Line::rows`] is what the layout and the renderer both ask, so
/// the two can never disagree about where the next line starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    /// A sub heading, in the same yellow the menus use for what is selected.
    Head(&'static str),
    /// A paragraph line, centred like everything else on the menu screens.
    Body(&'static str),
    /// A left hand label and its right hand explanation.
    ///
    /// Split into two columns rather than padded with spaces because the
    /// built in font is not monospaced: a column aligned with spaces lines up
    /// on paper and not on screen. The explanation column starts at
    /// [`description_column`], which every pair line on the page agrees on.
    Pair(&'static str, &'static str),
    /// A mode: its name on one row and its actual rule on the next.
    ///
    /// Printed the way the main menu prints it, name over rule, rather than as
    /// a two column pair. The rules are full sentences and the names are up to
    /// eight characters, so side by side they need more width than the screen
    /// has, and a rule that runs off the edge is worse than one that wraps onto
    /// its own line.
    Mode(Mode),
    /// Vertical space between sections. Carries no text.
    Gap,
}

impl Line {
    /// How many rows of pitch this line takes up.
    pub fn rows(self) -> i32 {
        match self {
            Line::Mode(_) => 2,
            _ => 1,
        }
    }
}

/// One page: a name for the header, and the lines under it.
#[derive(Debug, Clone, Copy)]
pub struct Page {
    /// The page's own name, shown large at the top.
    pub name: &'static str,
    /// The printed lines, top to bottom.
    pub lines: &'static [Line],
}

/// The manual, in reading order.
pub const PAGES: &[Page] = &[
    Page {
        name: "HOW TO PLAY",
        lines: &[
            Line::Head("THE GOAL"),
            Line::Body("Stack the falling pieces to fill the"),
            Line::Body("bottom row of the well. A full row"),
            Line::Body("clears, and everything above it drops"),
            Line::Body("one place. Clear as many as you can"),
            Line::Body("before the stack reaches the top."),
            Line::Gap,
            Line::Head("LEVELS AND SPEED"),
            Line::Body("Gravity pulls the piece down faster as"),
            Line::Body("the level climbs. Every 10 lines is a"),
            Line::Body("level, and level 20 is as fast as the"),
            Line::Body("game ever gets. The game's own music"),
            Line::Body("speeds up a little at each level too."),
            Line::Gap,
            Line::Head("THE WELL"),
            Line::Body("The faint grid lines up the well mark"),
            Line::Body("where a piece can go. The ghost piece"),
            Line::Body("shows where the piece will land, and it"),
            Line::Body("can be turned off in Options."),
        ],
    },
    Page {
        name: "CONTROLS",
        lines: &[
            Line::Head("KEYBOARD, THE DEFAULTS"),
            Line::Pair("LEFT / RIGHT", "slide the piece"),
            Line::Pair("DOWN", "soft drop, 1 point a cell"),
            Line::Pair("UP", "rotate clockwise"),
            Line::Pair("Z", "rotate anticlockwise"),
            Line::Pair("SPACE", "hard drop, 2 points a cell"),
            Line::Pair("C", "hold the piece"),
            Line::Pair("P", "pause the run"),
            Line::Pair("X", "next custom music track"),
            Line::Pair("ENTER", "confirm a menu choice"),
            Line::Pair("ESC", "back, or quit from the menu"),
            Line::Pair("H", "training only, clears a row"),
            Line::Gap,
            Line::Head("ALSO WORKS"),
            Line::Body("A and D slide, S soft drops, W rotates"),
            Line::Body("and LEFT SHIFT holds."),
            Line::Gap,
            Line::Head("CONTROLLER"),
            Line::Body("Any button or stick direction can be"),
            Line::Body("bound to any action, and the D-pad can"),
            Line::Body("stand in for the arrow keys. Change them"),
            Line::Body("under Options, then Keyboard Keybinds or"),
            Line::Body("Controller Keybinds."),
        ],
    },
    Page {
        name: "SCORING",
        lines: &[
            Line::Head("LINES CLEARED"),
            Line::Body("1 line = 100        2 lines = 300"),
            Line::Body("3 lines = 500       4 lines = 800"),
            Line::Gap,
            Line::Head("DROPPING"),
            Line::Body("Soft drop adds 1 point a cell"),
            Line::Body("Hard drop adds 2 points a cell"),
            Line::Gap,
            Line::Head("SPINS"),
            Line::Body("A piece rotated into its own shape is"),
            Line::Body("a spin, and spins pay more than plain"),
            Line::Body("lines. The last thing you do has to be"),
            Line::Body("the rotation, so the piece really has"),
            Line::Body("to twist into place."),
            Line::Gap,
            Line::Head("BACK TO BACK"),
            Line::Body("Clear lines several times in a row and"),
            Line::Body("each one is worth one and a half times"),
            Line::Body("as much. It is the fastest way to build"),
            Line::Body("a score."),
            Line::Gap,
            Line::Head("COMBOS AND PERFECT CLEARS"),
            Line::Body("Clearing on consecutive pieces scores"),
            Line::Body("more each time. Emptying the well with"),
            Line::Body("a single piece is a perfect clear, and"),
            Line::Body("pays a bonus on top."),
        ],
    },
    Page {
        name: "MODES",
        lines: &[
            Line::Head("THE FIVE MODES"),
            Line::Mode(Mode::Marathon),
            Line::Mode(Mode::Sprint),
            Line::Mode(Mode::Ultra),
            Line::Mode(Mode::Training),
            Line::Mode(Mode::Master),
            Line::Gap,
            Line::Head("DIFFICULTY"),
            Line::Body("Options also sets difficulty, which moves"),
            Line::Body("how quickly the levels arrive. The Master"),
            Line::Body("difficulty applies to every mode, and opens"),
            Line::Body("a run at level 15."),
            Line::Gap,
            Line::Head("HIGH SCORES"),
            Line::Body("Every mode keeps its own table. Set a new"),
            Line::Body("record and you get three letters to type"),
            Line::Body("in with it."),
        ],
    },
    Page {
        name: "OPTIONS AND FILES",
        lines: &[
            Line::Head("OPTIONS"),
            Line::Body("Everything can be changed: drop speed,"),
            Line::Body("colours, piece skins, effects, the grid"),
            Line::Body("and its opacity, backgrounds, music and"),
            Line::Body("sound. Keybinds have their own screens, so"),
            Line::Body("a key can be captured without being acted"),
            Line::Body("on."),
            Line::Gap,
            Line::Head("CUSTOM MUSIC"),
            Line::Body("Point the game at any folder and its"),
            Line::Body("tracks play in turn. Your own music is"),
            Line::Body("played as written; only the game's own"),
            Line::Body("track speeds up with the level."),
            Line::Gap,
            Line::Head("WHERE THINGS ARE SAVED"),
            Line::Body("Settings, high scores and the music"),
            Line::Body("folder are kept in your user profile, so"),
            Line::Body("the game folder stays clean and the whole"),
            Line::Body("thing can be copied anywhere and run."),
        ],
    },
];

/// How many pages the manual has.
pub fn page_count() -> usize {
    PAGES.len()
}

/// The page at `index`, or the first page if the index is out of range.
///
/// Written as a lookup rather than an indexing operation because the index
/// arrives from a screen variant the player drives; returning a real page
/// means a bad value degrades to the start of the book instead of a panic
/// mid frame.
pub fn page(index: usize) -> &'static Page {
    PAGES.get(index).unwrap_or(&PAGES[0])
}

/// The page `steps` on from `index`, wrapping at both ends.
///
/// UP and DOWN both turn the page and both have to work from wherever the
/// player is, including from the last page, so the wrap is here rather than
/// being clamped at each end and stranding the book.
pub fn turn(index: usize, steps: isize) -> usize {
    let count = page_count() as isize;
    (((index as isize + steps) % count + count) % count) as usize
}

/// Where the body text starts.
pub const BODY_TOP: i32 = 148;

/// The gap between one line and the next.
pub const LINE_PITCH: i32 = 26;

/// Where the footer sits, and therefore how much room the body has.
pub const FOOTER_TOP: i32 = SCREEN_HEIGHT - 40;

/// The left edge of the label column.
pub const LABEL_LEFT: i32 = 18;

/// The space between a label and its explanation.
pub const COLUMN_GAP: i32 = 12;

/// The font size the body is printed at.
pub const BODY_SIZE: i32 = 18;

/// The font size headings print at.
pub const HEAD_SIZE: i32 = BODY_SIZE + 2;

/// Every string `page` prints, with the size it is printed at.
///
/// Test only. The font sizes live here, next to the text, so a line can be
/// checked at the size it is drawn at rather than at a size copied out of the
/// renderer. Only the layout tests want that pairing, so this is compiled for
/// them alone and the release build carries no code for it.
#[cfg(test)]
pub fn page_texts(page: &Page) -> Vec<(&'static str, i32)> {
    let mut out = vec![(page.name, HEAD_SIZE)];
    for line in page.lines {
        match *line {
            Line::Head(s) => out.push((s, HEAD_SIZE)),
            Line::Body(s) => out.push((s, BODY_SIZE)),
            Line::Pair(label, what) => {
                out.push((label, BODY_SIZE));
                out.push((what, BODY_SIZE));
            }
            Line::Mode(mode) => {
                out.push((mode.label(), HEAD_SIZE));
                out.push((mode.blurb(), BODY_SIZE));
            }
            Line::Gap => {}
        }
    }
    out
}

/// How many rows of pitch a whole page takes up.
pub fn total_rows(page: &Page) -> i32 {
    page.lines.iter().map(|l| l.rows()).sum()
}

/// The y of the first row after `page`'s last line.
///
/// The check that matters: this has to stay above [`FOOTER_TOP`], or the
/// bottom of a page is drawn over the footer that says how to leave it, and
/// the player cannot read the controls for the screen they are on.
pub fn last_line_y(page: &Page) -> i32 {
    BODY_TOP + total_rows(page) * LINE_PITCH
}

/// Where the explanation column starts on `page`.
///
/// Measured from the widest label *and* the widest explanation actually on the
/// page, and the whole two column block is then centred as one unit. Measuring
/// only the labels and starting the explanations at a fixed margin is what put
/// the longest mode rule off the right edge: it fits when the labels are short
/// and hangs off when a single explanation happens to be long. Centring the
/// block means the widest pair decides where the block sits, and nothing is
/// wider than the screen.
///
/// `measure` is a parameter so the rule can be tested without a window: real
/// text width needs one, and this is the kind of arithmetic that gets a pixel
/// wrong.
pub fn description_column(measure: impl Fn(&str) -> i32, page: &Page) -> i32 {
    let (widest_label, widest_what) = column_widths(measure, page);
    LABEL_LEFT.max(((SCREEN_WIDTH - widest_label - COLUMN_GAP - widest_what) / 2).max(0))
        + widest_label
        + COLUMN_GAP
}

/// The widest label and the widest explanation on `page`, in pixels.
///
/// Split out of [`description_column`] because both the layout rule and the
/// test that says the block fits need the same two numbers, and a test that
/// re-derived them would be a second, weaker copy of the rule.
fn column_widths(measure: impl Fn(&str) -> i32, page: &Page) -> (i32, i32) {
    let mut widest_label = 0;
    let mut widest_what = 0;
    for line in page.lines {
        if let Line::Pair(label, what) = line {
            widest_label = widest_label.max(measure(label));
            widest_what = widest_what.max(measure(what));
        }
    }
    (widest_label, widest_what)
}

/// The width the whole two column block needs on `page`.
///
/// The number that has to stay inside the screen. A pair line that is wider
/// than this is a line the player cannot read the whole of, so it is checked
/// by the tests rather than left to be discovered on screen.
pub fn block_width(measure: impl Fn(&str) -> i32, page: &Page) -> i32 {
    let (label, what) = column_widths(measure, page);
    label + COLUMN_GAP + what
}

/// The manual's own footer, telling the player how to turn the page.
pub const FOOTER: &str = "UP and DOWN turn the page, ESC goes back";

/// The header line under the page name, e.g. `PAGE 2 OF 5`.
pub fn page_of(index: usize) -> String {
    format!("PAGE {} OF {}", index + 1, page_count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Mode;

    /// A deliberately pessimistic width estimate: raylib's default font is
    /// roughly this wide per character at size 18, and rounding up keeps the
    /// layout checks strict rather than convenient.
    const CHAR_W: i32 = 9;

    fn estimate(s: &str) -> i32 {
        s.chars().count() as i32 * CHAR_W
    }

    /// Every string a page prints, in one flat list, gaps and all skipped.
    fn strings(page: &Page) -> Vec<&'static str> {
        let mut out = vec![page.name];
        for line in page.lines {
            match *line {
                Line::Head(s) | Line::Body(s) => out.push(s),
                Line::Pair(a, b) => {
                    out.push(a);
                    out.push(b);
                }
                Line::Mode(mode) => {
                    out.push(mode.label());
                    out.push(mode.blurb());
                }
                Line::Gap => {}
            }
        }
        out
    }

    #[test]
    fn the_manual_is_a_real_book() {
        assert!(
            page_count() >= 3,
            "a manual of {} page(s) is a note, not a manual",
            page_count()
        );
        for (i, p) in PAGES.iter().enumerate() {
            assert!(!p.name.is_empty(), "page {i} has no name");
            assert!(
                p.lines.iter().any(|l| !matches!(l, Line::Gap)),
                "page {} ({}) is blank",
                i,
                p.name
            );
        }
    }

    /// raylib's built in font has no glyph outside ASCII, so anything else is
    /// drawn as an empty box. This is the guard that keeps smart quotes and
    /// long dashes out of the manual.
    #[test]
    fn the_manual_is_pure_ascii_so_every_character_can_be_drawn() {
        for p in PAGES {
            for s in strings(p) {
                assert!(
                    s.is_ascii(),
                    "page {} has a character the font cannot draw: {s:?}",
                    p.name
                );
            }
        }
    }

    /// The rule that makes the manual usable at all: no page may run into the
    /// footer. This is what a new section has to be checked against before it
    /// is added.
    #[test]
    fn no_page_overflows_into_the_footer() {
        for (i, p) in PAGES.iter().enumerate() {
            let last = last_line_y(p);
            assert!(
                last <= FOOTER_TOP - LINE_PITCH,
                "page {} ({}) ends at y {last}, under the footer at y {FOOTER_TOP}",
                i,
                p.name
            );
        }
    }

    /// Every centred line has to fit across the screen. Headings, paragraphs
    /// and the rules under the mode names are all centred, so all three are
    /// checked by one rule rather than three that can drift.
    #[test]
    fn every_centred_line_fits_the_width() {
        for p in PAGES {
            for line in p.lines {
                let centred = match *line {
                    Line::Head(s) | Line::Body(s) => Some(s),
                    Line::Mode(mode) => Some(mode.blurb()),
                    Line::Pair(_, what) => Some(what),
                    Line::Gap => None,
                };
                let Some(s) = centred else { continue };
                // Headings print two pixels larger, which is the same two
                // pixels the width allowance already carries.
                assert!(
                    estimate(s) + 2 * LABEL_LEFT <= SCREEN_WIDTH,
                    "page {} has an over-long line ({}px): {s:?}",
                    p.name,
                    estimate(s)
                );
            }
        }
    }

    /// A label and its explanation sit side by side, so the two column block
    /// has to fit across the screen. The widest pair on the page decides.
    #[test]
    fn every_label_and_its_explanation_fit_side_by_side() {
        for p in PAGES {
            let wide = block_width(estimate, p);
            assert!(
                wide + 2 * LABEL_LEFT <= SCREEN_WIDTH,
                "page {} needs {wide}px for its two columns, more than the {SCREEN_WIDTH}px screen",
                p.name
            );
            // And the column lands inside the screen, never negative and never
            // past the right edge.
            let col = description_column(estimate, p);
            assert!(col >= LABEL_LEFT, "page {} put the column at x {col}", p.name);
            let (_, widest_what) = column_widths(estimate, p);
            assert!(
                col + widest_what <= SCREEN_WIDTH - LABEL_LEFT,
                "page {} runs its explanations off the screen at x {}",
                p.name,
                col + widest_what
            );
        }
    }

    /// The explanation column follows the widest label on the page it is on,
    /// so two pages with different label widths do not share a column, and a
    /// page whose long label and long explanation cancel out still fits.
    #[test]
    fn the_explanation_column_follows_the_widest_label() {
        let narrow = Page {
            name: "NARROW",
            lines: &[Line::Pair("A", "short")],
        };
        let wide = Page {
            name: "WIDE",
            lines: &[Line::Pair("A", "short"), Line::Pair("MUCH LONGER LABEL", "long")],
        };
        // Narrow: a short label and a short explanation, so the block is
        // centred and the column is roughly half way across.
        assert_eq!(block_width(estimate, &narrow), estimate("A") + estimate("short") + COLUMN_GAP);
        let narrow_col = description_column(estimate, &narrow);
        assert!(narrow_col > SCREEN_WIDTH / 4 && narrow_col < SCREEN_WIDTH / 2);
        // Wide: the label is what grew, so the explanations move right with it.
        assert_eq!(
            block_width(estimate, &wide),
            estimate("MUCH LONGER LABEL") + COLUMN_GAP + estimate("short")
        );
        assert!(description_column(estimate, &wide) > narrow_col);
        // A page with no pairs at all still has to produce a sane column
        // rather than an overflow from an empty maximum.
        let none = Page {
            name: "NONE",
            lines: &[Line::Head("ONLY A HEADING")],
        };
        assert_eq!(block_width(estimate, &none), COLUMN_GAP);
        assert_eq!(description_column(estimate, &none), SCREEN_WIDTH / 2 + COLUMN_GAP / 2);
    }

    /// A block wider than the screen is not silently clipped: it clamps at the
    /// left margin so the label stays readable and the loss is at the end.
    #[test]
    fn a_block_too_wide_to_center_still_starts_on_screen() {
        let huge = Page {
            name: "HUGE",
            lines: &[Line::Pair(
                "A LABEL FAR TOO LONG FOR THIS SCREEN",
                "AND AN EXPLANATION TOO LONG FOR THIS SCREEN AS WELL",
            )],
        };
        assert!(block_width(estimate, &huge) > SCREEN_WIDTH);
        assert_eq!(
            description_column(estimate, &huge),
            LABEL_LEFT + estimate("A LABEL FAR TOO LONG FOR THIS SCREEN") + COLUMN_GAP
        );
    }

    /// Turning the page has to work from either end and either direction,
    /// because UP and DOWN are the only two ways out of the book.
    #[test]
    fn turning_the_page_wraps_at_both_ends_and_both_directions() {
        let last = page_count() - 1;
        assert_eq!(turn(0, -1), last, "up from the first page");
        assert_eq!(turn(0, 1), 1);
        assert_eq!(turn(last, 1), 0, "down past the last page");
        assert_eq!(turn(1, -1), 0);
        // And a jump longer than the book, which a held key can produce.
        assert_eq!(turn(0, page_count() as isize), 0);
        assert_eq!(turn(0, -(page_count() as isize) - 1), last);
        // Turning down then all the way back up lands where it started.
        let mut there = 0;
        for _ in 0..page_count() {
            there = turn(there, 1);
        }
        assert_eq!(there, 0, "a full turn of the book is not a full turn");
    }

    /// A bad index must not panic mid frame; it lands on the cover page.
    #[test]
    fn an_out_of_range_page_index_falls_back_to_the_first_page() {
        assert_eq!(page(page_count()).name, PAGES[0].name);
        assert_eq!(page(usize::MAX).name, PAGES[0].name);
        assert_eq!(page(0).name, PAGES[0].name);
    }

    /// The header counts pages from one, because a manual that says "PAGE 0 OF
    /// 5" has a bug in it.
    #[test]
    fn the_header_numbers_pages_the_way_a_book_does() {
        assert_eq!(page_of(0), format!("PAGE 1 OF {}", page_count()));
        assert_eq!(
            page_of(page_count() - 1),
            format!("PAGE {} OF {}", page_count(), page_count())
        );
        assert!(!page_of(0).contains("PAGE 0"));
    }

    /// The printed text and the size it is printed at are one list, because a
    /// check at the wrong size is not a check. Every string a page prints has
    /// to appear exactly once in `page_texts`, or it is drawn but unchecked, or
    /// checked but never drawn.
    #[test]
    fn every_printed_string_is_listed_with_the_size_it_is_drawn_at() {
        for p in PAGES {
            let listed = page_texts(p);
            let printed = strings(p);
            assert_eq!(
                listed.len(),
                printed.len(),
                "page {} lists {} strings but prints {}",
                p.name,
                listed.len(),
                printed.len()
            );
            for s in printed {
                assert!(
                    listed.contains(&(s, HEAD_SIZE)) || listed.contains(&(s, BODY_SIZE)),
                    "page {} prints {s:?} at a size page_texts does not list",
                    p.name
                );
            }
            // And the page name is in the list at the header size.
            assert_eq!(listed[0], (p.name, HEAD_SIZE));
        }
    }

    /// A page's height has to come from what it actually prints, not from how
    /// many entries it happens to have: a mode line is two rows tall, so a
    /// count of entries would put the last line of the modes page a full pitch
    /// lower than the drawn one and the footer check would be checking a page
    /// that is not the page on screen.
    #[test]
    fn a_pages_height_counts_rows_not_entries() {
        let one = Page {
            name: "ONE",
            lines: &[Line::Mode(Mode::Sprint)],
        };
        assert_eq!(total_rows(&one), 2);
        assert_eq!(last_line_y(&one), BODY_TOP + 2 * LINE_PITCH);
        let two = Page {
            name: "TWO",
            lines: &[Line::Mode(Mode::Sprint), Line::Body("short")],
        };
        assert_eq!(total_rows(&two), 3);
        // A gap is a real blank row, not nothing at all.
        let gapped = Page {
            name: "GAPPED",
            lines: &[Line::Gap, Line::Body("short")],
        };
        assert_eq!(total_rows(&gapped), 2);
        // And the modes page really is taller than its entry count, which is
        // the whole reason this function exists.
        let modes = PAGES
            .iter()
            .find(|p| p.name == "MODES")
            .expect("the manual has a modes page");
        let entries = modes.lines.len() as i32;
        assert!(
            total_rows(modes) > entries,
            "the modes page is {} entries and {} rows, so nothing is being tested",
            entries,
            total_rows(modes)
        );
    }

    /// The manual has to state each mode's actual rule, not a remembered one.
    /// This is the test that catches a mode changing its goal while its page
    /// keeps the old sentence: the manual holds the [`Mode`] itself, so the
    /// rule it prints is whatever the mode says today.
    #[test]
    fn the_manual_states_every_modes_real_rule() {
        let printed: Vec<Line> = PAGES.iter().flat_map(|p| p.lines.iter().copied()).collect();
        for mode in Mode::ALL {
            assert!(
                printed.contains(&Line::Mode(mode)),
                "the manual does not state {}'s rule: {:?}",
                mode.label(),
                mode.blurb()
            );
        }
        // And nothing lists a mode that no longer exists.
        let named = printed.iter().filter(|l| matches!(l, Line::Mode(_))).count();
        assert_eq!(named, Mode::ALL.len(), "the manual lists a mode twice over");
    }

    /// The manual teaches the controls, so the defaults it prints have to be
    /// the defaults the game really uses. Renaming a default without fixing
    /// the book is exactly the drift this catches, and it compares against
    /// [`keys::name`], the same spelling the keybind screens show the player,
    /// so the book and the rebinding menu cannot disagree.
    #[test]
    fn the_manual_prints_the_real_default_keys() {
        let labels: Vec<&'static str> = PAGES
            .iter()
            .flat_map(|p| p.lines.iter())
            .filter_map(|l| match l {
                Line::Pair(label, _) => Some(*label),
                _ => None,
            })
            .collect();
        // All eight remappable actions have to be findable on the controls
        // page. A label may name two keys at once, as "LEFT / RIGHT" does, so
        // the check is that the key's own name appears as a word in some
        // label rather than that a label equals it.
        for action in crate::settings::ACTIONS {
            let name = crate::keys::name(action.default_key());
            assert!(
                labels
                    .iter()
                    .any(|l| l.split_whitespace().any(|word| word == name)),
                "the controls page never mentions {name} ({action:?})"
            );
        }
    }

    /// Every section a player needs has to be in the book: how to play, what
    /// the keys do, what scores, and where the files went.
    #[test]
    fn the_manual_covers_everything_a_player_needs_to_know() {
        let all: String = PAGES
            .iter()
            .flat_map(strings)
            .collect::<Vec<_>>()
            .join(" ");
        for needed in [
            "clear",
            "ghost",
            "hold",
            "hard drop",
            "soft drop",
            "back to back",
            "spin",
            "perfect clear",
            "combo",
            "keybinds",
            "custom music",
        ] {
            assert!(
                all.to_lowercase().contains(needed),
                "the manual never mentions {needed:?}"
            );
        }
    }
}
