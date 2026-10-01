//! Finding custom music: scanning a folder for tracks, the way the original did.
//!
//! The original had three pieces here: `get_music_files` (scan a folder for
//! audio), `update_custom_music_playlist` (fall back to the bundled track when
//! the setting is off or the folder is unusable), and a native folder picker.
//! The first two are ported below as-is.
//!
//! The picker is not: the original used `NSOpenPanel` on macOS and `zenity` on
//! Linux, neither of which exists on Windows, and raylib has no folder dialog of
//! its own. [`crate::folderpick`] opens the desktop's real folder dialog
//! instead - the same File Explorer window a player expects - and there is no
//! in-game browser left to fall back to.
//!
//! The scanning half is deliberately pure so the ordering rules - which the
//! original got quite specific about - can be tested without a filesystem.

use std::path::{Path, PathBuf};

/// The audio extensions the original accepted, verbatim.
pub const AUDIO_EXTENSIONS: [&str; 7] = ["wma", "wav", "ogg", "mp3", "aac", "flac", "m4a"];

/// The original capped a scan at 500 files so a folder of a few thousand
/// tracks could not stall the game.
pub const MAX_TRACKS: usize = 500;

/// Whether `name` ends in one of [`AUDIO_EXTENSIONS`], case-insensitively.
///
/// The original compared `os.path.splitext(...)[1].lower()`, so `Track.MP3`
/// matched and a name like `mp3` with no dot did not.
pub fn is_audio_file(name: &str) -> bool {
    match name.rsplit_once('.') {
        Some((stem, ext)) => {
            // `rsplit_once` gives the text after the last dot, but a leading-dot
            // name such as `.mp3` is a hidden file, not an mp3. The original
            // skipped dotfiles before ever reaching this test.
            !stem.is_empty() && AUDIO_EXTENSIONS.iter().any(|e| e.eq_ignore_ascii_case(ext))
        }
        None => false,
    }
}

/// The original's sort key: digit-leading names first, then names starting
/// with a letter, then everything else; ties broken case-insensitively.
///
/// This is what makes a folder of `01.mp3`, `02.mp3`, `B.MP3`, `alpha.ogg`
/// come out in that order rather than in raw filesystem order.
pub fn sort_key(name: &str) -> (u8, String) {
    let category = match name.chars().next() {
        Some(c) if c.is_ascii_digit() => 0,
        Some(c) if c.is_ascii_alphabetic() => 1,
        _ => 2,
    };
    (category, name.to_uppercase())
}

/// Order a list of filenames the way the original's `sort_key` did.
pub fn sorted_names(names: &mut [String]) {
    names.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
}

/// Every playable track directly inside `dir`.
///
/// Not recursive: the original only read the top level. Dotfiles are skipped
/// before the extension test, and the result is capped at [`MAX_TRACKS`].
/// A directory that cannot be read yields an empty list rather than an error,
/// so a bad path degrades to the bundled track instead of stopping the game.
pub fn collect_tracks(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        // `is_file` follows symlinks, matching `os.path.isfile`.
        if entry.path().is_file() {
            files.push(name);
        }
    }

    sorted_names(&mut files);
    files
        .into_iter()
        .filter(|name| is_audio_file(name))
        .map(|name| dir.join(name))
        .take(MAX_TRACKS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- extension matching ---------------------------------------------

    #[test]
    fn the_original_seven_extensions_are_accepted() {
        for ext in AUDIO_EXTENSIONS {
            let name = format!("track.{ext}");
            assert!(is_audio_file(&name), "{name} should be playable");
        }
    }

    #[test]
    fn extension_matching_ignores_case() {
        // The original lowercased the extension before comparing.
        assert!(is_audio_file("Track.MP3"));
        assert!(is_audio_file("TRACK.Ogg"));
        assert!(is_audio_file("a.FLAC"));
    }

    #[test]
    fn other_and_missing_extensions_are_rejected() {
        for name in ["cover.png", "notes.txt", "movie.mp4", "noextension", "song.wma.bak"] {
            assert!(!is_audio_file(name), "{name} should not be playable");
        }
    }

    #[test]
    fn a_dotfile_named_like_a_track_is_not_a_track() {
        // `.mp3` has no stem, and hidden files are skipped before this test
        // anyway; it must not be mistaken for a playable file.
        assert!(!is_audio_file(".mp3"));
    }

    #[test]
    fn a_dotted_name_still_matches_on_its_real_extension() {
        assert!(is_audio_file("Mr. Blue Sky.ogg"));
        assert!(!is_audio_file("Mr. Blue Sky.png"));
    }

    // --- ordering -------------------------------------------------------

    #[test]
    fn digit_names_sort_before_letters_and_letters_before_symbols() {
        let mut names: Vec<String> =
            ["zeta.mp3", "!bang.ogg", "01.mp3", "Alpha.wav", "_under.mp3", "9.mp3"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        sorted_names(&mut names);
        assert_eq!(
            names,
            vec!["01.mp3", "9.mp3", "Alpha.wav", "zeta.mp3", "!bang.ogg", "_under.mp3"]
        );
    }

    #[test]
    fn ordering_ignores_case() {
        let mut names: Vec<String> = ["b.mp3", "A.mp3", "a.mp3", "B.mp3"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        sorted_names(&mut names);
        // All four uppercase-fold to the same key, so any of them may lead; what
        // matters is that lowercase is not pushed to the end.
        assert_eq!(names[0].to_uppercase(), "A.MP3");
        assert_eq!(names[3].to_uppercase(), "B.MP3");
    }

    #[test]
    fn digit_names_sort_numerically_by_name_not_by_value() {
        // The original sorted by name, so "10" precedes "9". That is
        // surprising but it is what it did, so it is what this does.
        let mut names: Vec<String> = ["9.mp3", "10.mp3"].iter().map(|s| s.to_string()).collect();
        sorted_names(&mut names);
        assert_eq!(names, vec!["10.mp3", "9.mp3"]);
    }

    #[test]
    fn sort_key_category_matches_the_originals_three_buckets() {
        assert_eq!(sort_key("1a.mp3").0, 0);
        assert_eq!(sort_key("a1.mp3").0, 1);
        assert_eq!(sort_key("~x.mp3").0, 2);
    }

    // --- scanning a real folder ------------------------------------------

    /// A throwaway folder that cleans up after itself.
    ///
    /// Named after the test that asked for it so the parallel test threads
    /// cannot collide on the same directory.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("tetrafusion-scan-{tag}"));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("could not make a scratch folder");
            Self(dir)
        }

        fn file(&self, name: &str) -> &Self {
            let path = self.0.join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("could not make a scratch subfolder");
            }
            std::fs::write(path, b"").expect("could not write a scratch file");
            self
        }

        fn dir(&self, name: &str) -> &Self {
            std::fs::create_dir_all(self.0.join(name)).expect("could not make a scratch subfolder");
            self
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Just the file names, in the order `collect_tracks` produced them.
    fn names(tracks: &[PathBuf]) -> Vec<String> {
        tracks
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_real_folder_is_scanned_filtered_and_ordered() {
        let s = Scratch::new("scan");
        s.file("b.mp3")
            .file("01.ogg")
            .file("A.wav")
            .file("9.flac")
            .file(".hidden.mp3")
            .file("cover.png")
            .file("notes.txt")
            .dir("Album")
            .file("Album/inner.mp3");
        // Digits first, then letters, case-folded; dotfiles, other extensions
        // and the subfolder are all left out.
        assert_eq!(
            names(&collect_tracks(&s.0)),
            vec!["01.ogg", "9.flac", "A.wav", "b.mp3"]
        );
    }

    #[test]
    fn a_folder_of_more_than_five_hundred_tracks_is_capped() {
        let s = Scratch::new("cap");
        for i in 0..MAX_TRACKS + 20 {
            s.file(&format!("t{i:04}.mp3"));
        }
        assert_eq!(collect_tracks(&s.0).len(), MAX_TRACKS);
    }

    #[test]
    fn an_unreadable_folder_yields_no_tracks_rather_than_an_error() {
        // The setting can outlive its folder - a portable drive unplugged, a
        // settings file copied from another machine - and the game has to keep
        // playing the bundled track instead of giving up.
        assert!(collect_tracks(Path::new("Z:/nope/never")).is_empty());
    }
}