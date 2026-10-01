//! Every bundled asset, compiled into the executable.
//!
//! # Why this module exists
//!
//! The requirement is that `tetrafusion.exe` **is** the game: copy it anywhere,
//! double-click it, and it works. That means no `assets/` folder next to it, no
//! working-directory requirement, nothing to install. Before this module the
//! game reached its sounds and background photos by searching the working
//! directory and seven parent directories for `assets/<name>` and degrading to
//! silence/plain backdrop when that failed - which meant a copied-to-the-desktop
//! exe was a *worse* copy of the game than the one in `target/release`.
//!
//! So the files are `include_bytes!`-ed here and handed straight to raylib's
//! memory loaders. `find_asset` on disk is kept as a **first** choice in the
//! callers, not a last one: a player who drops their own `1.jpg` next to the exe
//! still gets their photo. What changed is that the embedded copy is the floor
//! rather than the ceiling - an asset on disk improves the game, and its absence
//! no longer costs anything.
//!
//! # What is and is not embedded
//!
//! * The 15 background photos and the 5 sounds: embedded, because the game is
//!   unrecognisable without them.
//! * `assets/ICON1.ico`: embedded, but by `build.rs` as a **resource section**,
//!   not as data - see that file for why a runtime call cannot work.
//! * `assets/tetris-blocks.TTF`: deliberately not embedded. Nothing in the game
//!   ever loads it; all text goes through raylib's built-in font, whose glyph
//!   coverage is what `effects.rs` documents. Embedding a file nothing reads
//!   would only make the exe bigger.
//! * Music the player supplies via Options is *not* bundled, by definition - it
//!   is a folder the player picks, and `music_dir` reads it from disk.
//!
//! # Adding an asset
//!
//! Add the `include_bytes!` line, then add a name to the matching test below.
//! The tests fail loudly rather than letting a typo ship a silent asset.

/// Filename -> bytes, for the sound effects and the bundled music track.
///
/// `ogg` is the extension raylib's memory loaders are handed; it is derived from
/// each name's extension rather than repeated, so a renamed file cannot end up
/// decoded with the wrong decoder.
pub const AUDIO: &[(&str, &[u8])] = &[
    ("Background.ogg", include_bytes!("../assets/Background.ogg")),
    ("GAMEOVER.ogg", include_bytes!("../assets/GAMEOVER.ogg")),
    (
        "Lineclear.ogg",
        include_bytes!("../assets/Lineclear.ogg"),
    ),
    (
        "MultipleLineclear.ogg",
        include_bytes!("../assets/MultipleLineclear.ogg"),
    ),
    (
        "heartbeat_grid_almost_full.ogg",
        include_bytes!("../assets/heartbeat_grid_almost_full.ogg"),
    ),
];

/// Level number -> bytes, for the background photos, in the order they load.
///
/// Numbered rather than a flat list because the loader walks `1.jpg`,
/// `2.jpg`, ... and stops at the first number that is neither on disk nor
/// embedded, so a player with three extra photos on disk gets 18 images in
/// order rather than 15 plus three orphans.
pub const BACKGROUNDS: &[(u32, &[u8])] = &[
    (1, include_bytes!("../assets/backgrounds/1.jpg")),
    (2, include_bytes!("../assets/backgrounds/2.jpg")),
    (3, include_bytes!("../assets/backgrounds/3.jpg")),
    (4, include_bytes!("../assets/backgrounds/4.jpg")),
    (5, include_bytes!("../assets/backgrounds/5.jpg")),
    (6, include_bytes!("../assets/backgrounds/6.jpg")),
    (7, include_bytes!("../assets/backgrounds/7.jpg")),
    (8, include_bytes!("../assets/backgrounds/8.jpg")),
    (9, include_bytes!("../assets/backgrounds/9.jpg")),
    (10, include_bytes!("../assets/backgrounds/10.jpg")),
    (11, include_bytes!("../assets/backgrounds/11.jpg")),
    (12, include_bytes!("../assets/backgrounds/12.jpg")),
    (13, include_bytes!("../assets/backgrounds/13.jpg")),
    (14, include_bytes!("../assets/backgrounds/14.jpg")),
    (15, include_bytes!("../assets/backgrounds/15.jpg")),
];

/// The bundled music track's filename, as [`Audio::try_music`] asks for it.
pub const MUSIC: &str = "Background.ogg";

/// The extension for `name`, without the dot, for diagnostics and the format
/// tests.
///
/// Not what the memory loaders want - see [`IMAGE_FORMAT`] and [`AUDIO_FORMAT`].
#[cfg(test)]
pub fn extension(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((_, ext)) => ext,
        None => "",
    }
}

/// The format string handed to `LoadImageFromMemory` for a background photo.
///
/// # Why a constant, and why the dot
///
/// raylib compares the format string with `strcmp` against **dotted** literals -
/// `strcmp(fileType, ".jpg") == 0` in the vendored source. It does not accept a
/// bare `"jpg"` and it does not call `GetFileExtension`, so passing the result of
/// `name.rsplit_once('.')` fails with `IMAGE: Data format not supported` and
/// hands back a null image, which then silently drops the photo.
///
/// Measured, not assumed: with `"jpg"` every embedded background failed while
/// `LoadTexture` on a temp file of the same bytes succeeded. The dot was the
/// entire difference.
///
/// `memory_loader_formats_are_dotted_and_type_specific` keeps this from being
/// undone, and from being reused for the wrong kind of file.
pub const IMAGE_FORMAT: &str = ".jpg";

/// The format string handed to `LoadWaveFromMemory` / `LoadMusicStreamFromMemory`
/// for a sound.
///
/// A separate constant from [`IMAGE_FORMAT`] rather than one shared value: the
/// two loaders are handed different files, and `LoadWaveFromMemory` matches
/// `.wav`, `.ogg`, `.mp3`, `.flac` and `.xm` only - handing it `.jpg` fails just
/// as silently as the missing dot did.
pub const AUDIO_FORMAT: &str = ".ogg";

/// The bytes for a bundled sound, by filename.
///
/// `None` for anything not compiled in, which the callers treat as "fall back to
/// looking on disk".
pub fn audio(name: &str) -> Option<&'static [u8]> {
    AUDIO
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, bytes)| *bytes)
}

/// The bytes for background photo `n` (1-based).
pub fn background(n: u32) -> Option<&'static [u8]> {
    BACKGROUNDS
        .iter()
        .find(|(num, _)| *num == n)
        .map(|(_, bytes)| *bytes)
}

/// The highest background number that is compiled in.
pub fn background_count() -> u32 {
    BACKGROUNDS
        .iter()
        .map(|(n, _)| *n)
        .max()
        .unwrap_or(0)
}

/// Every embedded asset, by its `assets/`-relative name.
///
/// Test-only: it exists so a test can prove the embedded set has not drifted
/// from the files in `assets/`.
#[cfg(test)]
pub fn embedded_names() -> Vec<String> {
    let mut names: Vec<String> = AUDIO.iter().map(|(n, _)| (*n).to_string()).collect();
    names.extend(
        BACKGROUNDS
            .iter()
            .map(|(n, _)| format!("backgrounds/{n}.jpg")),
    );
    names.sort();
    names
}

/// Total bytes of asset data compiled in.
///
/// A plain function rather than a `const fn`: summing over slices in const
/// context is not stable, and the only caller is the test that catches an
/// emptied include list.
#[cfg(test)]
pub fn embedded_bytes() -> usize {
    AUDIO.iter().map(|(_, b)| b.len()).sum::<usize>()
        + BACKGROUNDS.iter().map(|(_, b)| b.len()).sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assets_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
    }

    /// The headline guarantee: the exe is the game. This fails the moment an
    /// asset is added to `assets/` and not embedded, which is the regression
    /// that made a copied exe run silent.
    #[test]
    fn every_loadable_asset_on_disk_is_embedded() {
        let dir = assets_dir();
        let embedded = embedded_names();
        let mut missing = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("assets/ exists").flatten() {
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            let file = path.file_name().unwrap().to_string_lossy().into_owned();
            // ICON1.ico is a build resource, not runtime data.
            if file == "ICON1.ico" || file.ends_with(".TTF") {
                continue;
            }
            if !embedded.iter().any(|n| *n == file) {
                missing.push(file);
            }
        }
        for entry in std::fs::read_dir(dir.join("backgrounds"))
            .expect("assets/backgrounds exists")
            .flatten()
        {
            let file = entry.path().file_name().unwrap().to_string_lossy().into_owned();
            if !embedded.iter().any(|n| *n == format!("backgrounds/{file}")) {
                missing.push(format!("backgrounds/{file}"));
            }
        }
        assert!(
            missing.is_empty(),
            "these assets are on disk but not compiled in, so the exe would \
             ship without them: {missing:?}"
        );
    }

    /// Nothing embedded may be a zero-length file: `include_bytes!` happily
    /// accepts one and raylib's decoders fail at runtime with no useful message.
    #[test]
    fn no_embedded_asset_is_empty() {
        for (name, bytes) in AUDIO {
            assert!(!bytes.is_empty(), "{name} embedded as 0 bytes");
        }
        for (n, bytes) in BACKGROUNDS {
            assert!(!bytes.is_empty(), "background {n}.jpg embedded as 0 bytes");
        }
    }

    /// An Ogg stream starts with `OggS`; a JPEG starts with `FF D8 FF`. Checking
    /// the magic bytes is the cheapest way to catch a file that is present but
    /// is not the format its extension claims - which would decode to garbage.
    #[test]
    fn embedded_files_have_the_right_magic_bytes() {
        for (name, bytes) in AUDIO {
            assert_eq!(
                &bytes[..4],
                b"OggS",
                "{name} is not an Ogg stream; check the include_bytes! path"
            );
        }
        for (n, bytes) in BACKGROUNDS {
            assert_eq!(
                &bytes[..3],
                &[0xFF, 0xD8, 0xFF],
                "{n}.jpg is not a JPEG; check the include_bytes! path"
            );
        }
    }

    /// Every embedded name needs the extension its loader is given.
    #[test]
    fn extension_is_derived_and_present() {
        assert_eq!(extension("Lineclear.ogg"), "ogg");
        assert_eq!(extension("backgrounds/12.jpg"), "jpg");
        assert_eq!(extension("no-extension"), "");
        for (name, _) in AUDIO {
            assert!(!extension(name).is_empty(), "{name} has no extension");
        }
    }

    /// raylib's `*FromMemory` loaders `strcmp` the format string against
    /// **dotted** literals (`".jpg"`, `".ogg"`). Passing a bare extension makes
    /// every embedded asset fail to decode with `Data format not supported`,
    /// which is exactly the bug this assertion exists to keep out.
    ///
    /// The second half is the follow-on: there is one format string per *kind* of
    /// asset, not one shared value. Handing `LoadWaveFromMemory` the image format
    /// fails in exactly the same silent way.
    #[test]
    fn memory_loader_formats_are_dotted_and_type_specific() {
        assert!(
            IMAGE_FORMAT.starts_with('.') && !IMAGE_FORMAT.ends_with('.'),
            "IMAGE_FORMAT is {IMAGE_FORMAT:?}; raylib strcmps it against \".jpg\" \
             and friends, so the leading dot is required and a trailing one is not"
        );
        assert!(
            [".jpg", ".jpeg", ".png", ".bmp", ".tga"].contains(&IMAGE_FORMAT),
            "IMAGE_FORMAT is {IMAGE_FORMAT:?}, which no background photo uses"
        );
        assert!(
            AUDIO_FORMAT.starts_with('.') && !AUDIO_FORMAT.ends_with('.'),
            "AUDIO_FORMAT is {AUDIO_FORMAT:?}; raylib strcmps it against \".ogg\" \
             and friends, so the leading dot is required and a trailing one is not"
        );
        assert!(
            [".ogg", ".wav", ".mp3", ".flac", ".xm"].contains(&AUDIO_FORMAT),
            "AUDIO_FORMAT is {AUDIO_FORMAT:?}, which LoadWaveFromMemory does not decode"
        );
        assert_ne!(
            IMAGE_FORMAT, AUDIO_FORMAT,
            "the two memory loaders must not share one format string"
        );
        // Every embedded asset has to match the format it will be decoded as.
        for (n, _) in BACKGROUNDS {
            assert_eq!(
                extension(&format!("backgrounds/{n}.jpg")),
                IMAGE_FORMAT.trim_start_matches('.'),
                "background {n}.jpg does not match IMAGE_FORMAT"
            );
        }
        for (name, _) in AUDIO {
            assert_eq!(
                extension(name),
                AUDIO_FORMAT.trim_start_matches('.'),
                "{name} does not match AUDIO_FORMAT"
            );
        }
    }

    /// Background numbers must be a gapless `1..=count` run, because the loader
    /// stops at the first gap. A hole would silently drop every photo after it.
    #[test]
    fn background_numbers_are_gapless_from_one() {
        assert_eq!(background_count(), BACKGROUNDS.len() as u32);
        for n in 1..=background_count() {
            assert!(background(n).is_some(), "background {n} is missing");
        }
        assert!(background(0).is_none());
        assert!(background(background_count() + 1).is_none());
    }

    /// The lookups used by the callers must return what they claim to.
    #[test]
    fn lookups_find_the_bundled_assets() {
        assert!(audio(MUSIC).is_some());
        assert_eq!(
            audio("Lineclear.ogg"),
            AUDIO.iter().find(|(n, _)| *n == "Lineclear.ogg").map(|(_, b)| *b)
        );
        assert!(audio("NotARealSound.ogg").is_none());
    }

    /// A rough size floor. This is not "how big should the exe be"; it is a
    /// tripwire for the silent regression where every `include_bytes!` gets
    /// dropped and the game quietly loses its content, which compiles fine.
    #[test]
    fn the_embedded_set_is_not_trivially_small() {
        assert!(
            embedded_bytes() > 5_000_000,
            "only {} bytes of assets are compiled in; the include list was \
             probably emptied",
            embedded_bytes()
        );
    }
}