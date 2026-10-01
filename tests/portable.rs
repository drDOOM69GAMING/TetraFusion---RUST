//! Proves the promise on the tin: **the exe is the game.**
//!
//! Every other test in this crate runs against `src/`, with `assets/` sitting on
//! disk a few directories away. That is comfortable, and it is exactly why the
//! shipping requirement went unnoticed for so long: nothing ever asked the only
//! question that matters to a player, which is what happens when the exe is
//! copied somewhere with no `assets/` folder next to it.
//!
//! Two real bugs are pinned by this file, and both of them passed 409 unit
//! tests, compiled without a warning, and printed nothing:
//!
//! * **The format string.** `LoadImageFromMemory` and `LoadWaveFromMemory`
//!   `strcmp` their format argument against `".jpg"` / `".ogg"` *with a leading
//!   dot*. Passing the bare extension fails with `Data format not supported` and
//!   returns null, so every embedded background and every sound silently
//!   vanished and the game fell back to its on-disk lookup - which does not exist
//!   in the folder under test.
//! * **The music buffer.** `LoadMusicStreamFromMemory` does not copy the bytes.
//!   For Ogg it parks a `stb_vorbis` handle that reads straight out of the
//!   caller's buffer in `music.ctxData`, so passing a temporary `Vec` left every
//!   `UpdateMusicStream` reading freed memory. The symptom was a process pinned
//!   at 100% CPU making no progress and never taking its screenshot.
//!
//! Both are silent at compile time and invisible to a unit test, which is why
//! this drives the real binary and checks the files it leaves behind.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The freshly linked binary. `CARGO_BIN_EXE_*` is available to integration
/// tests only, which is why this lives in `tests/`.
fn built_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tetrafusion"))
}

/// An empty directory somewhere disposable, so the run cannot accidentally find
/// the project's `assets/` by walking up the tree. It is created under the
/// system temp directory rather than inside the workspace for exactly that
/// reason: `find_asset` searches parents, and a temp folder has none of ours.
fn empty_dir(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("tetrafusion_portable_{tag}"));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("could not create the temp folder");
    p
}

/// What one autonomous smoke run left behind.
struct Run {
    stderr: String,
    grid: Option<String>,
    exit: Option<i32>,
    timed_out: bool,
}

/// Copy the exe into `dir`, run it there with `TF_SMOKE=1`, and collect the
/// result. Nothing else is placed in the folder.
fn run_portable(dir: &Path) -> Run {
    let exe = built_exe();
    let exe_name = exe
        .file_name()
        .expect("exe has a name")
        .to_string_lossy()
        .into_owned();
    let copied = dir.join(&exe_name);
    std::fs::copy(&exe, &copied).expect("could not copy the exe into the temp folder");

    // The folder must contain the exe and nothing else, or the test proves
    // nothing: with an `assets/` alongside it, every asset would load from disk
    // and the embedded path would never be exercised.
    let others: Vec<String> = std::fs::read_dir(dir)
        .expect("temp folder should be readable")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| *n != exe_name)
        .collect();
    assert!(
        others.is_empty(),
        "the temp folder must start empty apart from the exe, found: {others:?}"
    );

    let mut child = Command::new(&copied)
        .current_dir(dir)
        .env("TF_SMOKE", "1")
        .env("TF_WINSIZE", "819x930")
        .env("TF_MODE", "marathon")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("could not start the executable");

    // A generous ceiling: the run needs a desktop session to open a window, and
    // on a slow machine that is slow. What it must never do is never finish -
    // that is precisely the music-buffer failure, which spins forever at 100%.
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut timed_out = false;
    let status = loop {
        match child.try_wait().expect("could not poll the process") {
            Some(status) => break Some(status),
            None if Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                timed_out = true;
                break None;
            }
            None => std::thread::sleep(Duration::from_millis(250)),
        }
    };
    if timed_out {
        return Run {
            stderr: String::new(),
            grid: None,
            exit: None,
            timed_out: true,
        };
    }
    let mut out = String::new();
    if let Some(mut s) = child.stdout.take() {
        use std::io::Read;
        let _ = s.read_to_string(&mut out);
    }
    let mut err = String::new();
    if let Some(mut s) = child.stderr.take() {
        use std::io::Read;
        let _ = s.read_to_string(&mut err);
    }
    Run {
        stderr: format!("{err}{out}"),
        grid: std::fs::read_to_string(dir.join("smoke_grid.txt")).ok(),
        exit: status.map(|s| s.code().unwrap_or(-1)),
        timed_out: false,
    }
}

/// The headline requirement: copied to an empty folder, the game plays with its
/// backgrounds and its sound, and exits on its own.
#[test]
fn the_exe_alone_is_a_complete_game() {
    let dir = empty_dir("alone");
    let run = run_portable(&dir);

    assert!(
        !run.timed_out,
        "the game never finished from an empty folder. It was left spinning at \
         100% CPU. The usual cause is a buffer handed to \
         LoadMusicStreamFromMemory that the caller then dropped - raylib keeps a \
         pointer into it rather than copying."
    );

    assert_eq!(
        run.exit,
        Some(0),
        "the game did not exit cleanly (code {:?}). stderr:\n{}",
        run.exit,
        run.stderr
    );

    let grid = run
        .grid
        .expect("the smoke run wrote no smoke_grid.txt, so the game never reached a frame it could report on");
    assert!(
        grid.contains("mode=Marathon"),
        "the panel dump does not look like a Marathon run:\n{grid}"
    );
    // The smoke run hard-drops one piece per frame until it has placed 12, so a
    // completed run must say so. Parsed rather than substring-matched: `pieces=1`
    // would also match `pieces=12`.
    let pieces = grid
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().find(|f| f.starts_with("pieces=")))
        .and_then(|f| f.trim_start_matches("pieces=").parse::<u32>().ok())
        .unwrap_or_else(|| panic!("no pieces= field in the panel dump:\n{grid}"));
    assert!(
        pieces >= 12,
        "the smoke run only placed {pieces} piece(s); the game loop is not \
         advancing:\n{grid}"
    );

    // Nothing may have fallen back to "not found".
    for bad in [
        "no images found",
        "could not load",
        "will run silent",
        "none available",
        "Data format not supported",
    ] {
        assert!(
            !run.stderr.contains(bad),
            "the run from an empty folder reported {bad:?}, so something was not \
             embedded in the executable:\n{}",
            run.stderr
        );
    }

    // And the proof that it is the *embedded* copies being used: the background
    // line names how many came from the executable.
    assert!(
        run.stderr.contains("compiled into the executable"),
        "expected the background loader to report that it used the embedded \
         copies; stderr:\n{}",
        run.stderr
    );
}

/// A screenshot of a real playfield from that folder, not just a clean exit.
/// A run that bails out before drawing anything would otherwise pass.
#[test]
fn the_exe_alone_draws_the_game() {
    let dir = empty_dir("draws");
    let run = run_portable(&dir);
    assert!(!run.timed_out, "the game never finished from an empty folder");

    let shot = dir.join("smoke.png");
    let bytes = std::fs::read(&shot)
        .unwrap_or_else(|e| panic!("no screenshot was taken in {}: {e}", dir.display()));
    assert!(
        bytes.len() > 20_000,
        "the screenshot is only {} bytes, which is a blank window rather than a \
         rendered board",
        bytes.len()
    );
    // A PNG, not something else written to the same name.
    assert!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "smoke.png is not a PNG"
    );
}

/// The asset payload really is in the file. This is the cheap half of the check
/// and it does not need a window, so it holds even on a machine with no display:
/// 15 photos at ~1.3 MB each plus the sound set is far more than the executable
/// was before they were embedded, and a size floor catches an emptied include
/// list that still compiles cleanly.
#[test]
fn the_executable_contains_the_asset_payload() {
    let exe = built_exe();
    let bytes = std::fs::read(&exe).expect("built exe should be readable");

    // A distinctive run of bytes from the middle of one background photo. Taken
    // from the file on disk rather than hard-coded, so it tracks the asset if it
    // is ever replaced.
    let photo =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/backgrounds/7.jpg"))
            .expect("backgrounds/7.jpg should be in the repo");
    let marker = &photo[photo.len() / 2..photo.len() / 2 + 32];

    let copies = bytes.windows(marker.len()).filter(|w| *w == marker).count();
    assert!(
        copies >= 1,
        "a 32-byte run from the middle of backgrounds/7.jpg does not appear in \
         the executable, so the backgrounds are not compiled into it. The exe is \
         {} MB; it should be carrying roughly 20 MB of assets.",
        bytes.len() / (1024 * 1024)
    );
}