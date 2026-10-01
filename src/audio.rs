//! Sound effects and background music, mirroring the original's asset set.
//!
//! `Background.ogg` loops while a run is live, `Lineclear.ogg` sounds for
//! one-to-three-row clears, `MultipleLineclear.ogg` for a Tetris,
//! `GAMEOVER.ogg` at the end of a run, and `heartbeat_grid_almost_full.ogg`
//! pulses while the top rows are occupied - exactly the triggers the Python
//! original used.
//!
//! raylib-rs ties every `Sound`/`Music` to the audio device that loaded it,
//! so the whole set borrows the `RaylibAudio` handle. Every load is
//! fallible: a missing or unreadable asset silently disables that sound
//! rather than stopping the game, and a machine with no audio device at all
//! simply runs silent.

use std::path::{Path, PathBuf};

use raylib::audio::{Music, RaylibAudio, Sound};

use crate::assets;
use crate::config::gravity_g;
use crate::game::{Event, Game};
use crate::music_dir::collect_tracks;
use crate::settings::Settings;

/// Resolve an asset under `assets/` on disk.
///
/// Only reached when a sound is not compiled in, or when a player has put their
/// own copy of a file next to the exe - disk wins where it exists, because a
/// replacement is a deliberate act, and the embedded copy is the floor that
/// makes a copied-to-the-desktop exe complete rather than silent.
///
/// Searches the working directory and the executable's folder, walking up to
/// seven parent levels from each. That covers `cargo run` (cwd is the crate
/// root), a double-clicked exe inside `target/debug` (two floors up there is
/// the crate-root `assets/`), and everything in between.
pub(crate) fn find_asset(name: &str) -> String {
    let mut starts: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            starts.push(dir.to_path_buf());
        }
    }

    for start in starts {
        let mut dir = Some(start);
        for _ in 0..7 {
            let Some(d) = dir else { break };
            let candidate = d.join("assets").join(name);
            if candidate.try_exists().unwrap_or(false) {
                return candidate.to_string_lossy().into_owned();
            }
            dir = d.parent().map(|p| p.to_path_buf());
        }
    }
    format!("assets/{name}")
}

fn try_sound<'a>(device: &'a RaylibAudio, name: &str) -> Option<Sound<'a>> {
    if let Some(sound) = sound_from_memory(device, name) {
        return Some(sound);
    }
    match device.new_sound(&find_asset(name)) {
        Ok(sound) => Some(sound),
        Err(err) => {
            eprintln!("audio: could not load {name}: {err}");
            None
        }
    }
}

/// Decode a sound effect out of the executable and hand it to the device.
///
/// raylib's C API has no `LoadSoundFromMemory`; the documented route is
/// `LoadWaveFromMemory` then `LoadSoundFromWave`. `LoadSoundFromWave` **takes
/// ownership of the wave and unloads it** before returning - it copies the
/// samples into a fresh audio stream and then calls `UnloadWave` on what it was
/// given. raylib-rs's `Wave` wrapper still believes it owns that memory and
/// would free it a second time on drop, so the wrapper is `forget`-ten on
/// purpose: the memory is already gone and must not be freed again.
///
/// Returning `None` for an asset that is not compiled in leaves the caller to
/// look on disk.
fn sound_from_memory<'a>(device: &'a RaylibAudio, name: &str) -> Option<Sound<'a>> {
    let bytes = assets::audio(name)?;
    let wave = device
        .new_wave_from_memory(assets::AUDIO_FORMAT, bytes)
        .map_err(|e| format!("{name} (embedded): {e}"))
        .map_or_else(
            |e| {
                eprintln!("audio: {e}");
                None
            },
            Some,
        )?;
    let sound = device.new_sound_from_wave(&wave).ok()?;
    // See the doc comment: `LoadSoundFromWave` already unloaded this wave.
    std::mem::forget(wave);
    Some(sound)
}

/// Load the bundled background track.
///
/// It keeps raylib's default `looping = true`, because this one track *is*
/// meant to repeat for the whole run - that is what the original did with it.
///
/// # The buffer has to be owned by the caller
///
/// `LoadMusicStreamFromMemory` **does not copy** the bytes. For an Ogg it calls
/// `stb_vorbis_open_memory(data, ...)` and parks the resulting handle in
/// `music.ctxData`, which reads straight out of the caller's buffer on every
/// `UpdateMusicStream`. So the `Vec` passed here cannot be a temporary: it is
/// pushed onto `bufs` and owned by [`Audio::music_bytes`] for as long as the
/// returned `Music` can be played.
///
/// Handing it a temporary `&bytes.to_vec()` compiled, ran, printed no error at
/// all, and then pinned a core at 100% with the game loop making no progress,
/// because every frame was reading a freed allocation. `cargo test` was green
/// throughout. This is the single most dangerous thing in the file, which is why
/// it is spelled out here and why `tests/portable.rs` drives the real binary
/// until it exits on its own.
fn try_music<'a>(device: &'a RaylibAudio, bufs: &mut Vec<Vec<u8>>) -> Option<Music<'a>> {
    if let Some(bytes) = assets::audio(assets::MUSIC) {
        // Pushed before the load, so it cannot be dropped between the call and
        // the first `UpdateMusicStream`. Old entries are left in place rather
        // than cleared: freeing one while the `Music` that reads it is still
        // alive is exactly the bug above, and this is called at most a handful
        // of times per session.
        bufs.push(bytes.to_vec());
        match device.new_music_from_memory(assets::AUDIO_FORMAT, bufs.last().expect("just pushed"))
        {
            Ok(music) => return Some(music),
            Err(err) => eprintln!("audio: embedded {} failed: {err}", assets::MUSIC),
        }
    }
    match device.new_music(&find_asset(assets::MUSIC)) {
        Ok(music) => Some(music),
        Err(err) => {
            eprintln!("audio: could not load {}: {err}", assets::MUSIC);
            None
        }
    }
}

/// Turn a custom track's repeat off, so the playlist can advance past it.
///
/// This is the whole reason the playlist was stuck. `LoadMusicStream` sets
/// `Music.looping = true` at load time, and `UpdateMusicStream` only calls
/// `StopMusicStream` when a track runs out `if (!music.looping)`; otherwise it
/// wraps `framesProcessed` back around with `framesProcessed % frameCount`.
/// Either way `is_stream_playing()` never goes false, so the "the track ended,
/// play the next one" branch in [`Audio::keep_music_playing`] was unreachable
/// and a picked folder repeated one song forever.
///
/// raylib-rs's `Music` is a thin wrapper that derefs to `ffi::Music`, so the
/// flag is written straight through the wrapper.
fn set_looping(music: &mut Music<'_>, looping: bool) {
    music.looping = looping;
}

/// What the music driver does about one frame of playback.
///
/// Split out of [`Audio::keep_music_playing`] because the interesting part is
/// a decision, not a raylib call: whether a playlist track has ended is
/// inferred from "it was playing a moment ago and is not now", and that
/// inference is what the repeat bug got wrong. Testing it as a pure function
/// of what the driver can observe keeps it honest without an audio device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MusicAction {
    /// No playlist: the bundled track should be going. (It loops itself, so
    /// restarting it when it is merely paused is correct.)
    EnsurePlaying,
    /// A custom track is still playing; leave it alone.
    Wait,
    /// A custom track that was armed has stopped, which means it reached its
    /// end. Move to the next one.
    Advance,
    /// A custom track is loaded but stopped and was never armed: the first
    /// play, or a restart after music was switched back on. Start it, but do
    /// not read that as a track ending - that would skip a song per toggle.
    Start,
}

/// How much faster the bundled track plays for each level reached.
///
/// Level one is the track at its normal tempo, and every level after that adds
/// this much speed. The count is [`config::gravity_g`] rather than the raw
/// level number, because gravity is the thing that is actually getting faster:
/// it is one G more per level, so one G is one step of music. Tying the two
/// together is what makes the music track the game rather than merely climb.
///
/// An amount, not a factor, and the reason is the shape of the ladder. Speed is
/// heard as a ratio, so a factor gives every level the same size of jump - but
/// only if the factor is small enough to still be music at level twenty, and at
/// that size it is inaudible at level two, which is where a player spends most
/// of a run. An amount shrinks in ratio terms as the ramp climbs (this is a shade
/// under two and a half semitones at level two and about one at level twenty)
/// but stays a real step the whole way, and staying audibly a step is the
/// property that matters: the level change has to be heard.
///
/// The ramp before this was a 4% factor per level, about 69 cents. Gravity
/// doubles from level one to level two, so 69 cents is nowhere near the jump the
/// player just felt in the pieces. Levels arrive every [`config::LINES_PER_LEVEL`]
/// lines, so a step arrived far too rarely and far too small, and the track could
/// climb from level one to the level where gravity stops without the player
/// noticing a single step.
pub const MUSIC_SPEED_STEP: f32 = 0.30;

/// How the per-G step is bent as the ramp climbs.
///
/// Under 1, so the ramp's *shape* matches gravity's without matching its
/// magnitude. Gravity goes 1, 2, 3, 4... G, so the game itself accelerates
/// fastest at the bottom of the range; this keeps the music doing the same
/// thing, moving decisively early and easing off later.
///
/// A straight line per G - which is what the previous 0.15 step was - has the
/// opposite shape. It moved 15% at level 2 and the same 15% at level 19, so it
/// lagged the game's own acceleration badly at exactly the levels where the
/// player can feel the game speeding up, and kept pushing at levels where the
/// game was barely changing.
pub const MUSIC_SPEED_CURVE: f32 = 0.765;

/// The ceiling on the speed multiplier.
///
/// A marathon run has no end, so level 100 is reachable and would otherwise ask
/// for a hundred-fold speed, which is a chirp rather than music.
///
/// This is 4.0 rather than the 2.5 it used to be, and that came from a bad call.
/// At 2.5 the ramp hit the ceiling at level 11, which left levels 11 to 20 - nine
/// levels in which the pieces visibly speed up and the track does not move at all.
/// A player in that stretch hears a song that changed once and stopped, which is
/// the opposite of tracking the game. 4.0 puts the ceiling past level 20, so the
/// ramp never flattens anywhere gravity is still climbing.
///
/// The price is that the late game runs at up to 3.85 times speed, which is high
/// and pitchy. That is a deliberate trade and the alternative was worse: nine
/// levels of the hardest part of the game with a silent ramp.
pub const MUSIC_SPEED_MAX: f32 = 4.0;

/// The first level at which [`MUSIC_SPEED_MAX`] actually starts capping the ramp.
///
/// Worked out from the two constants above rather than written down by hand, so
/// that changing either one moves it instead of quietly leaving a wrong number in
/// a comment and a test. Only the tests need to know it: the driver itself has no
/// reason to ask where the cap is, because it compares the wanted speed against
/// the applied one and a capped level simply stops differing.
#[cfg(test)]
pub fn music_ceiling_level() -> i32 {
    // Searched rather than solved. The ramp is a power curve, so there is no
    // expression to invert: `(MAX - 1) / STEP` was the answer for the old linear
    // ramp and reported the cap at level 11 when the curve actually clears it. A
    // search cannot be wrong about its own shape, and the range is bounded by
    // gravity's own ceiling, so this stays a handful of iterations.
    let mut level = 1;
    while music_speed_for_level(level + 1) < MUSIC_SPEED_MAX - 1e-4 {
        level += 1;
        if level > 1000 {
            // Unreachable while the ramp is bounded, since `gravity_g` clamps.
            // Written as a literal rather than as `MAX_GRAVITY` on purpose: the
            // search has to be able to run past gravity's ceiling to find where
            // the cap actually lands, so this bounds the search rather than
            // second-guessing what the ramp should do.
            break;
        }
    }
    level + 1
}

/// A cached speed that cannot be a real speed, meaning "nothing has been pushed
/// to the stream yet, so push regardless".
const UNSET_SPEED: f32 = -1.0;

/// The playback speed the bundled track should run at for a given level.
///
/// Deliberately a pure function of the level so the ramp is testable without an
/// audio device, and so the value is a function of the level rather than of how
/// the level was reached: starting a Master run at level 15 has to sound like
/// level 15, not like level 1 played 14 times.
pub fn music_speed_for_level(level: i32) -> f32 {
    // Driven by gravity, so the music and the pieces climb the same ladder and
    // there is one number to change if either is retuned. `gravity_g` clamps at
    // the gravity ceiling, so level 1000 asks for the level 20 speed rather than
    // for something enormous.
    let g = (gravity_g(level) - 1) as f32;
    (1.0 + g.powf(MUSIC_SPEED_CURVE) * MUSIC_SPEED_STEP).min(MUSIC_SPEED_MAX)
}

/// What to hand raylib so the stream ends up running at `want`.
///
/// raylib's `SetMusicPitch` does not set a speed, it scales one. `SetAudioBufferPitch`
/// divides the converter's *current* output sample rate by whatever value it is given
/// and writes the quotient back, so two calls multiply instead of the second
/// replacing the first. Sending the level's speed directly therefore drifts further
/// from the intended tempo the longer a run lasts, and the level can no longer be
/// re-derived from what was asked for once the drift has happened.
///
/// Dividing the target by what has already been applied makes the accumulated
/// product land exactly on `want`, and makes the stream self correcting: every level
/// re-derives the rate from the intended speed rather than from a running total.
///
/// `applied` is [`UNSET_SPEED`] before anything has been pushed. That is treated as a
/// rate of 1.0, which is where a freshly loaded stream starts, so the first push is
/// the whole speed.
pub fn pitch_step(want: f32, applied: f32) -> f32 {
    let base = if applied > 0.0 { applied } else { 1.0 };
    want / base
}

/// The value to push for a level, or `None` when the level has not changed.
///
/// Split out of [`Audio::set_level_pitch`] so the choice of *what* to push is
/// testable without an audio device. The `None` case is what keeps
/// `SetMusicPitch` off the per-frame path.
pub fn next_pitch(want: f32, applied: f32) -> Option<f32> {
    if (want - applied).abs() < f32::EPSILON {
        return None;
    }
    Some(pitch_step(want, applied))
}

/// Decide this frame's music action from what the driver can observe.
fn music_action(playlist_empty: bool, playing: bool, track_armed: bool) -> MusicAction {
    if playlist_empty {
        return MusicAction::EnsurePlaying;
    }
    if playing {
        return MusicAction::Wait;
    }
    if track_armed {
        MusicAction::Advance
    } else {
        MusicAction::Start
    }
}

/// The playlist index after `current`, wrapping back to the start.
///
/// The original's `load_next_track` was `(current + 1) % len`, so skipping
/// past the last track starts the playlist again. Free-standing because it is
/// the one piece of the playlist that can be checked without an audio device.
fn next_track(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (current + 1) % len
    }
}

/// The full audio set the game can play.
pub struct Audio<'a> {
    /// Kept so a custom track can be loaded later: raylib ties every `Music`
    /// to the device that created it, so the device reference has to outlive
    /// each swap.
    device: &'a RaylibAudio,
    music: Option<Music<'a>>,
    /// Owns the byte buffers that any `music` loaded from the executable is
    /// reading out of.
    ///
    /// `LoadMusicStreamFromMemory` keeps a pointer into the buffer it was given
    /// rather than copying it, so this has to outlive the stream. Declared after
    /// `music` so the stream is dropped first, which is the order the two need.
    /// See [`try_music`].
    music_bytes: Vec<Vec<u8>>,
    line_clear: Option<Sound<'a>>,
    multi_line_clear: Option<Sound<'a>>,
    game_over: Option<Sound<'a>>,
    heartbeat: Option<Sound<'a>>,
    music_enabled: bool,
    heartbeat_playing: bool,
    game_over_played: bool,
    /// Custom-music state. An empty playlist means "play the bundled track on
    /// repeat", which is the original's fallback whenever `use_custom_music`
    /// is off or the chosen folder yielded nothing.
    playlist: Vec<PathBuf>,
    track: usize,
    /// True once a custom track is playing. Distinguishes "the track ended,
    /// advance" from "music is stopped right now", which both look the same to
    /// `is_stream_playing`.
    track_armed: bool,
    /// True while a custom track is loaded in music, so falling back to the
    /// bundled track knows whether it actually has to reload it.
    playing_custom: bool,
    /// The speed the bundled track was last told to play at, so the value is
    /// only pushed when it actually changes.
    ///
    /// `SetMusicPitch` has to be applied to the same `Music` the next level is
    /// going to play; custom tracks are never pitched, so this only tracks the
    /// bundled one.
    applied_speed: f32,
}

impl<'a> Audio<'a> {
    /// Load every asset and tie it to the given audio device. Failed loads
    /// degrade to silence for that asset only.
    pub fn new(device: &'a RaylibAudio, settings: &Settings) -> Self {
        // Built before the struct so the music stream's backing buffer is owned
        // by the same value that owns the stream.
        let mut music_bytes: Vec<Vec<u8>> = Vec::new();
        let music = try_music(device, &mut music_bytes);
        let mut audio = Self {
            device,
            music,
            music_bytes,
            line_clear: try_sound(device, "Lineclear.ogg"),
            multi_line_clear: try_sound(device, "MultipleLineclear.ogg"),
            game_over: try_sound(device, "GAMEOVER.ogg"),
            heartbeat: try_sound(device, "heartbeat_grid_almost_full.ogg"),
            music_enabled: settings.music_enabled,
            heartbeat_playing: false,
            game_over_played: false,
            playlist: Vec::new(),
            track: 0,
            track_armed: false,
            playing_custom: false,
            // The stream has not been pitched yet, so the first level change
            // is always pushed.
            applied_speed: UNSET_SPEED,
        };
        audio.apply_music_settings(settings);
        if audio.music.is_none()
            && audio.line_clear.is_none()
            && audio.multi_line_clear.is_none()
            && audio.game_over.is_none()
            && audio.heartbeat.is_none()
        {
            eprintln!(
                "audio: nothing could be loaded, not even the copies compiled into the \
                 executable. The game will run silent."
            );
        }
        audio
    }

    /// Rebuild the playlist from the current settings and jump to its first
    /// track, matching the original's `update_custom_music_playlist`.
    ///
    /// An unset, unreadable or empty folder falls back to the bundled track
    /// rather than leaving the game silent.
    pub fn apply_music_settings(&mut self, settings: &Settings) {
        let dir = settings.music_directory.trim();
        let tracks = if !settings.use_custom_music {
            Vec::new()
        } else if dir.is_empty() {
            // Custom music is on but no folder was ever chosen. The original
            // reported this and fell back the same way.
            eprintln!("audio: custom music is on but no folder is set");
            Vec::new()
        } else if !Path::new(dir).is_dir() {
            eprintln!("audio: '{dir}' is not a folder; using the bundled track");
            Vec::new()
        } else {
            let found = collect_tracks(Path::new(dir));
            if found.is_empty() {
                eprintln!("audio: no playable audio in '{dir}'; using the bundled track");
            }
            found
        };

        self.playlist = tracks;
        self.track = 0;
        self.track_armed = false;
        if self.playlist.is_empty() {
            self.restore_bundled_music();
        } else {
            self.play_track(0);
        }
    }

    /// Go back to looping `Background.ogg`.
    ///
    /// A no-op when no custom track is loaded, so start-up does not read the
    /// bundled file twice and re-reading unchanged settings is free.
    fn restore_bundled_music(&mut self) {
        if !self.playing_custom {
            return;
        }
        self.playing_custom = false;
        // Two statements rather than one: the new stream has to be built before
        // the old one is dropped, or the drop would free the buffer the old
        // stream is still reading.
        let music = try_music(self.device, &mut self.music_bytes);
        self.music = music;
        // A freshly loaded stream starts at pitch 1.0 whatever the level is, so
        // the cached value is dropped to force the ramp to be re-applied. Left
        // stale it would match a level that happens to want 1.0 and the track
        // would quietly stay slow.
        self.applied_speed = UNSET_SPEED;
    }

    /// What a custom playlist is currently doing, for the Options screen.
    ///
    /// `None` when the bundled track is playing, which is what the row falls
    /// back to describing. Positions are 1-based because that is how a person
    /// counts tracks.
    pub fn now_playing(&self) -> Option<(String, usize, usize)> {
        let path = self.playlist.get(self.track)?;
        let name = path.file_name()?.to_string_lossy().into_owned();
        Some((name, self.track + 1, self.playlist.len()))
    }

    /// Jump to the next track, as the Skip Track key and the gamepad button do.
    ///
    /// Does nothing when music is off or the bundled track is playing, matching
    /// the original's `skip_current_track`.
    pub fn skip(&mut self) {
        if !self.music_enabled || self.playlist.is_empty() {
            return;
        }
        self.advance();
    }

    /// The index of the track after the current one, wrapping.
    fn next_index(&self) -> usize {
        next_track(self.track, self.playlist.len())
    }

    /// Load and start a track, falling through to the next one if it will not
    /// decode. Returns false when every track in the playlist has failed.
    fn play_track(&mut self, index: usize) -> bool {
        if self.playlist.is_empty() {
            self.restore_bundled_music();
            return false;
        }
        // Bound the walk so a folder of, say, .wma files on a build without
        // wma support cannot spin here for every frame.
        let attempts = self.playlist.len().min(8);
        for step in 0..attempts {
            let i = (index + step) % self.playlist.len();
            let path = self.playlist[i].clone();
            match self.device.new_music(&path.to_string_lossy()) {
                Ok(mut music) => {
                    // A playlist track plays once, end to end. Leaving raylib's
                    // `looping = true` in place is what made a picked folder
                    // repeat one song forever; see `set_looping`.
                    set_looping(&mut music, false);
                    self.music = Some(music);
                    self.track = i;
                    self.track_armed = true;
                    self.playing_custom = true;
                    if let Some(m) = &self.music {
                        m.play_stream();
                    }
                    return true;
                }
                Err(err) => {
                    eprintln!("audio: skipping '{path:?}': {err}");
                }
            }
        }
        // Nothing in the playlist decodes. Rather than retrying every frame,
        // give up on the folder and fall back to the bundled track.
        eprintln!("audio: no track in the playlist could be loaded; using the bundled track");
        self.playlist.clear();
        self.track = 0;
        self.track_armed = false;
        self.restore_bundled_music();
        false
    }

    /// Move to the next track.
    fn advance(&mut self) {
        self.play_track(self.next_index());
    }

    /// Apply a music on/off change from the options screen immediately:
    /// stop the stream when disabling, resume when enabling.
    pub fn set_music_enabled(&mut self, enabled: bool) {
        self.music_enabled = enabled;
        if let Some(m) = &self.music {
            if enabled {
                m.update_stream();
                if !m.is_stream_playing() {
                    m.play_stream();
                }
            } else {
                m.stop_stream();
            }
        }
    }

    /// Whether any music asset is loaded at all (used by the options screen
    /// to show the real state when the asset is missing).
    pub fn music_available(&self) -> bool {
        self.music.is_some()
    }

    /// Per-frame driver. Feed it the live game (if any) and whether the
    /// game-over screen is up; it reacts to this frame's events and the
    /// state of the stack.
    pub fn update(&mut self, game: Option<&Game>, over: bool) {
        // The ramp is driven by the level, so it moves only while a run is
        // live, and it is applied before the stream is updated.
        self.set_level_pitch(game.map(|g| g.level));
        self.keep_music_playing(over);

        if over {
            // The moment the run ends: one game-over sting, silence on the
            // heartbeat, and no more line-clear effects.
            if !self.game_over_played {
                if let Some(s) = &self.game_over {
                    s.play();
                }
                if let Some(s) = &self.heartbeat {
                    s.stop();
                }
                self.heartbeat_playing = false;
                self.game_over_played = true;
            }
            return;
        }
        self.game_over_played = false;

        let Some(g) = game else { return };

        // Line clears: the original used the single-clear sound for anything
        // up to three rows and the multi-clear sound for a Tetris.
        for ev in g.events.iter() {
            if let Event::LineClear { rows, .. } = ev {
                if rows.len() == 4 {
                    if let Some(s) = &self.multi_line_clear {
                        s.play();
                    }
                } else if let Some(s) = &self.line_clear {
                    s.play();
                }
            }
        }

        // Heartbeat while the top four rows are occupied: restart it whenever
        // the track finishes, and stop the moment the danger clears.
        let danger = g.grid.danger_zone_active(4);
        if danger {
            if let Some(s) = &self.heartbeat {
                if !s.is_playing() {
                    s.play();
                }
            }
            self.heartbeat_playing = true;
        } else if self.heartbeat_playing {
            if let Some(s) = &self.heartbeat {
                s.stop();
            }
            self.heartbeat_playing = false;
        }
    }

    /// Speed the bundled track up as the level climbs, and put it back to
    /// normal speed whenever the bundled track is the one playing.
    ///
    /// Custom music is deliberately never pitched. The player chose those
    /// files; speeding them up with the level would quietly alter music they
    /// picked to hear as written, and a track that is 60% faster is a
    /// different recording. So the ramp is scoped to the game's own track.
    ///
    /// `None` (no live run) resets to 1.0, which is what the menus want.
    fn set_level_pitch(&mut self, level: Option<i32>) {
        if self.playing_custom {
            return;
        }
        let want = match level {
            Some(level) => music_speed_for_level(level),
            None => 1.0,
        };
        // Comparing before writing keeps this off the hot path: at 60 fps
        // `SetMusicPitch` would otherwise be called every frame of a level the
        // player is sitting still on.
        if (want - self.applied_speed).abs() < f32::EPSILON {
            return;
        }
        let Some(step) = next_pitch(want, self.applied_speed) else {
            return;
        };
        // raylib's `SetMusicPitch`; 1.0 is base level. Changing this while
        // the stream is playing is safe: it takes effect on the next
        // `UpdateMusicStream`, which is the very next thing the driver does.
        //
        // `applied_speed` is only recorded once the value has actually been
        // pushed. Recording it either way meant that a frame with no stream
        // loaded cached a speed that had never reached the audio device, and a
        // stream that appeared later would sit at 1.0 forever because the cached
        // value matched the level and nothing pushed again.
        if let Some(m) = &self.music {
            m.set_pitch(step);
            self.applied_speed = want;
        }
    }

    /// Keep music going while a run is live (and on the menus); go silent on
    /// the game-over screen until the player leaves it.
    ///
    /// The two sources behave differently, exactly as the original had them:
    /// the bundled `Background.ogg` loops forever, while a custom playlist
    /// plays one track at a time and advances when each one ends.
    fn keep_music_playing(&mut self, over: bool) {
        if !self.music_enabled || over {
            if let Some(m) = &self.music {
                m.stop_stream();
            }
            // Disarm so that re-enabling music restarts the current track
            // instead of reading the stop as "this track finished".
            self.track_armed = false;
            return;
        }

        let playing = match &self.music {
            Some(m) => {
                m.update_stream();
                m.is_stream_playing()
            }
            None => false,
        };

        match music_action(self.playlist.is_empty(), playing, self.track_armed) {
            MusicAction::EnsurePlaying => {
                if !playing {
                    if let Some(m) = &self.music {
                        m.play_stream();
                    }
                }
            }
            MusicAction::Wait => {}
            MusicAction::Advance => {
                // The current custom track ran to its end (it was not looping,
                // so the stream really stopped); on to the next, which is how
                // the original advanced on its music-ended event.
                self.advance();
            }
            MusicAction::Start => {
                if let Some(m) = &self.music {
                    m.play_stream();
                }
                self.track_armed = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        music_action, music_ceiling_level, music_speed_for_level, next_pitch, next_track,
        MusicAction, MUSIC_SPEED_MAX, UNSET_SPEED,
    };
    // The parent imports `gravity_g` by name but not the module, and `use
    // super::...` above lists names one by one. Named here so the tests can ask
    // where gravity stops instead of hardcoding 20 and drifting from it.
    use crate::config;

    /// raylib does not treat a pitch as an absolute speed.
    ///
    /// `SetMusicPitch` calls `SetAudioBufferPitch`, which divides the converter's
    /// *current* output sample rate by whatever it is handed and writes the result
    /// back. Successive calls therefore multiply rather than replace. This models
    /// that so the driver's arithmetic can be checked against it without an audio
    /// device.
    fn rate_after(steps: &[f32]) -> f32 {
        steps.iter().product()
    }

    /// Handing raylib the level's speed directly does not produce the level's
    /// speed; it runs away from it.
    ///
    /// This is the bug the ratio in [`next_pitch`] exists to prevent, kept as a
    /// test so the arithmetic stays visible: at the levels below, sending absolute
    /// speeds leaves the stream running at more than double what level 20 asks
    /// for, because every level multiplies the one before it.
    #[test]
    fn absolute_pitches_compound_which_is_why_the_driver_sends_a_ratio() {
        let levels = [1, 2, 3, 5, 8, 12, 20];
        let naive: Vec<f32> = levels.iter().map(|&l| music_speed_for_level(l)).collect();
        let wanted = music_speed_for_level(*levels.last().unwrap());
        assert!(
            (rate_after(&naive) - wanted).abs() > 0.5,
            "raylib's own arithmetic says sending absolute speeds compounds, but \
             the naive product {} came out near the wanted {wanted}",
            rate_after(&naive)
        );
    }

    /// Walking up through levels, the speed the stream actually ends up running at
    /// is the speed the current level asks for.
    ///
    /// This drives the same [`next_pitch`] the driver uses and replays raylib's
    /// multiplication, so it covers the value production hands over rather than a
    /// restatement of the formula.
    #[test]
    fn the_stream_ends_up_running_at_the_speed_the_level_asks_for() {
        let levels = [1, 2, 3, 5, 8, 12, 20, 30];
        let mut applied = UNSET_SPEED;
        let mut pushed: Vec<f32> = Vec::new();
        for &level in &levels {
            let want = music_speed_for_level(level);
            let Some(step) = next_pitch(want, applied) else {
                continue;
            };
            pushed.push(step);
            applied = want;
            let running = rate_after(&pushed);
            assert!(
                (running - want).abs() < 1e-3,
                "at level {level} the stream was running at {running} instead of {want}"
            );
        }
    }

    /// Sitting on one level must not push a pitch every frame.
    #[test]
    fn an_unchanged_level_pushes_nothing() {
        let want = music_speed_for_level(7);
        assert_eq!(next_pitch(want, want), None);
    }

    /// A level change always pushes, and the very first push is the whole speed
    /// rather than a ratio against a stream that has not been pitched yet.
    #[test]
    fn the_first_push_is_the_whole_speed() {
        let want = music_speed_for_level(1);
        assert_eq!(next_pitch(want, UNSET_SPEED), Some(1.0));
    }

    /// A single level up has to be clearly audible, at every level a run lives in.
    ///
    /// The step used to be 2% a level, and before that a 4% factor. Both are under
    /// the threshold of hearing when a level arrives every
    /// [`crate::config::LINES_PER_LEVEL`] lines, so the ramp was real, climbed the
    /// whole way, and read as nothing happening at all. A ramp nobody can hear per
    /// level is the same as no ramp.
    ///
    /// Checked across the early levels rather than at level one on purpose, because
    /// that is where the requirement has to hold: this is a curve now, not a line,
    /// so a single level's step tells you nothing about the rest of the ramp.
    ///
    /// The late levels are covered separately by `each_level_is_an_audible_step_
    /// that_shrinks_as_the_ramp_climbs`, which pins a lower floor for them
    /// together with the requirement that they keep shrinking. Splitting the bound
    /// this way is deliberate and is the price of matching the game's shape: the
    /// tail of the curve runs at about 3% a level, which no single step can make
    /// audible, but the levels a player actually reaches are the early ones where
    /// the step is 30%.
    #[test]
    fn a_single_level_up_is_audible_on_its_own() {
        // The first ten levels: ten lines apiece is a hundred lines of play, and
        // it is where the game's own acceleration is steepest.
        for level in 1..10 {
            let ratio = music_speed_for_level(level + 1) / music_speed_for_level(level);
            assert!(
                ratio >= 1.05 - 1e-4,
                "level {} to {} is only {:.1}% faster, which is below hearing",
                level,
                level + 1,
                (ratio - 1.0) * 100.0
            );
        }
        // And the first level up specifically, which is the one a player hears
        // most often: the game's fall speed doubles here, so the track has to
        // make a move that is unmistakably bigger than the old 4%.
        let first = music_speed_for_level(2) / music_speed_for_level(1);
        assert!(
            first >= 1.25,
            "level 1 to 2 is only {:.1}% faster",
            (first - 1.0) * 100.0
        );
    }

    /// The cap has to stay out of the whole range gravity climbs.
    ///
    /// Gravity is one G per level and stops at level 20, so a ramp that reaches
    /// its ceiling before then leaves the hardest part of the game - levels 11 to
    /// 20, where the pieces visibly accelerate - playing to a track that has
    /// stopped moving. That was reported as "the song ticks up once and stays at
    /// that same speed", and it was true from level 11 onwards.
    ///
    /// This used to only require the cap to stay past level 10, on the reasoning
    /// that most players never see the end of a run. That was wrong: the flat
    /// stretch was still reachable by anyone who got there, and the complaint was
    /// about the music stopping following the game rather than about how far a
    /// typical run goes. So the cap now has to clear the whole of gravity.
    #[test]
    fn the_cap_does_not_bind_before_gravity_stops_climbing() {
        let ceiling_level = music_ceiling_level();
        assert!(
            ceiling_level > config::MAX_GRAVITY as i32,
            "the cap binds at level {ceiling_level}, so the music stops moving at level \
             {} while gravity is still climbing",
            ceiling_level
        );
    }

    /// The bundled track is at normal speed on level one, and every level after
    /// that up to the cap is faster than the one before it.
    #[test]
    fn the_bundled_track_starts_at_normal_speed_and_rises_each_level() {
        assert_eq!(music_speed_for_level(1), 1.0);
        // Every level gravity actually climbs. Past `MAX_GRAVITY` the pieces
        // stop accelerating too - `gravity_g` clamps - so the ramp going flat
        // alongside them is the ramp still tracking the game.
        for level in 1..config::MAX_GRAVITY as i32 {
            assert!(
                music_speed_for_level(level + 1) > music_speed_for_level(level),
                "level {} was not faster than level {level}",
                level + 1
            );
        }
    }

    /// Each level moves the track by a clear step, and the steps get smaller.
    ///
    /// Two things at once, and both matter. Every step has to be big enough to
    /// hear - the old ramp's failure, and the reason the sound read as not
    /// following the game at all. And the steps have to shrink as the ramp
    /// climbs, because gravity does: the game doubles its fall speed from level
    /// one to two and then barely changes by level twenty, so a ramp whose steps
    /// stayed level would be loudest exactly where the player feels least.
    ///
    /// The floor is deliberately not the 4% that `a_single_level_up_is_audible_
    /// on_its_own` pins: the tail of this curve runs at about 3% per level, which
    /// is below a threshold a single level can clear but is still a continuous
    /// rise rather than a stop. That is the cost of matching the game's shape,
    /// and it is worth it, because the levels a player actually spends time in
    /// are the early ones where the step is 30%.
    #[test]
    fn each_level_is_an_audible_step_that_shrinks_as_the_ramp_climbs() {
        let mut previous = f32::MAX;
        for level in 1..config::MAX_GRAVITY as i32 {
            let here = music_speed_for_level(level);
            let next = music_speed_for_level(level + 1);
            let jump = (next - here) / here;
            assert!(
                jump >= 0.02,
                "level {level} to {} moved only {jump:.3}, which is not a step",
                level + 1
            );
            assert!(
                jump <= previous,
                "level {level} to {} jumped {jump:.3}, more than the {previous:.3} \
                 step before it, so the ramp is speeding up as gravity eases off",
                level + 1
            );
            previous = jump;
        }
    }

    /// The ramp has to have climbed by the time gravity peaks, and still be climbing.
    ///
    /// An amount step used to be allowed to reach the ceiling before gravity ran
    /// out, on the argument that holding at 2.5 beat climbing to 3.85. That
    /// argument chose an early plateau over a nine-level stretch where the pieces
    /// accelerate and the track does not. The plateau is gone and so is the
    /// excuse: the ramp is now required to still be moving at the level where
    /// gravity stops, with headroom left before the cap.
    #[test]
    fn the_ramp_is_still_climbing_where_gravity_stops() {
        let at_ten = music_speed_for_level(10);
        assert!(at_ten > 2.0, "level 10 was only {at_ten}");
        let at_twenty = music_speed_for_level(config::MAX_GRAVITY as i32);
        assert!(
            at_twenty > at_ten,
            "level {} came in at {at_twenty}, no faster than level 10's {at_ten}",
            config::MAX_GRAVITY
        );
        assert!(
            at_twenty < MUSIC_SPEED_MAX,
            "level {} is {at_twenty} and should still be short of the {MUSIC_SPEED_MAX} \
             ceiling",
            config::MAX_GRAVITY
        );
    }

    /// A Master run opens at level 15, so the ramp has to be a function of the
    /// level and not a count of level-ups. Otherwise Master would open at the
    /// sound of level 1.
    #[test]
    fn the_speed_follows_the_level_not_the_number_of_level_ups() {
        // Level 15 reached by starting there and level 15 reached by climbing
        // from 1 have to be identical, because Master does exactly the former.
        assert_eq!(music_speed_for_level(15), music_speed_for_level(15));
        // And it must already be noticeably up at Master start, not near 1.0.
        let at_master = music_speed_for_level(15);
        assert!(
            at_master > 1.20,
            "level 15 was only {at_master}, so a Master run would open at normal speed"
        );
    }

    /// Very high levels stay music rather than turning into a chirp.
    ///
    /// The ceiling is a safety net, not something the ramp is meant to reach.
    /// Gravity stops climbing at `MAX_GRAVITY`, so a ramp driven by gravity also
    /// stops there, at about 3.85 - under the cap but not far under. Raising the
    /// cap can never make the late game louder or faster; it only matters if
    /// gravity is ever retuned to climb further.
    #[test]
    fn the_ramp_stays_under_the_ceiling_however_high_the_level_goes() {
        for level in [20, 21, 100, 1000, i32::MAX] {
            let speed = music_speed_for_level(level);
            assert!(
                speed <= MUSIC_SPEED_MAX,
                "level {level} asked for {speed}, above the {MUSIC_SPEED_MAX} ceiling"
            );
        }
        // And the ramp genuinely tops out rather than climbing without limit: the
        // level where gravity stops is the level where the music stops too.
        assert_eq!(
            music_speed_for_level(1000),
            music_speed_for_level(config::MAX_GRAVITY as i32),
            "the ramp kept climbing past the level where gravity stops"
        );
    }

    /// The music's shape has to match the game's, not just its range.
    ///
    /// The complaint behind this was that the sound "only ticks twice" and does
    /// not follow the game. Gravity accelerates fastest at the bottom of the
    /// range - level 1 to 2 doubles the fall speed, level 5 to 6 adds 17% - so
    /// the music has to move most at the bottom too. The previous ramp moved the
    /// same 15% at every level, which meant it barely registered the doubling at
    /// level 2 and then carried on pushing hard at level 19, where the game had
    /// almost stopped changing.
    ///
    /// Both halves of that are asserted here: the early step must be clearly
    /// larger than the late one, and the early one must be substantial enough to
    /// hear at all.
    #[test]
    fn the_ramp_follows_the_shape_of_gravity() {
        let early = music_speed_for_level(2) / music_speed_for_level(1) - 1.0;
        let late = music_speed_for_level(20) / music_speed_for_level(19) - 1.0;
        assert!(
            early > 0.20,
            "level 1 to 2 is only {:.1}% faster, so the doubling of the game's own \
             fall speed is not audible in the track",
            early * 100.0
        );
        // Compared as a multiple rather than a difference, because that is what
        // "the same shape" means and the difference is too weak to tell the two
        // apart. A straight line per G (`MUSIC_SPEED_CURVE` of 1.0) makes the
        // first step 30% and the last 8%, a multiple of under 4. The curve has to
        // be clearly more than that or it is not bending at all.
        assert!(
            early > late * 5.0,
            "level 1 to 2 moves {early:.3} but level 19 to 20 moves {late:.3}, only \
             {:.1}x apart; a straight line per G would give {:.1}x, so the ramp is \
             not following the shape of gravity",
            early / late,
            0.30 / (0.30 / 3.853)
        );
    }

    /// A level below one, which should never happen but could if a mode
    /// started oddly, must not pitch the track below normal or invert it.
    #[test]
    fn a_nonsensical_level_does_not_slow_the_track_down() {
        assert_eq!(music_speed_for_level(0), 1.0);
        assert_eq!(music_speed_for_level(-5), 1.0);
    }

    #[test]
    fn skipping_past_the_last_track_starts_the_playlist_again() {
        // The original wrapped with `% len`, so Skip on track 3 of 3 goes
        // back to track 1 rather than stopping.
        assert_eq!(next_track(0, 3), 1);
        assert_eq!(next_track(1, 3), 2);
        assert_eq!(next_track(2, 3), 0);
    }

    #[test]
    fn a_single_track_playlist_skips_to_itself() {
        assert_eq!(next_track(0, 1), 0);
    }

    #[test]
    fn an_empty_playlist_does_not_divide_by_zero() {
        assert_eq!(next_track(0, 0), 0);
        assert_eq!(next_track(7, 0), 0);
    }

    #[test]
    fn walking_the_whole_playlist_returns_to_the_start() {
        // Whatever the length, a full lap must land back on the first track:
        // that is what makes a custom playlist loop rather than dead-end.
        for len in 1..=25usize {
            let mut i = 0;
            for _ in 0..len {
                i = next_track(i, len);
                assert!(i < len, "index {i} escaped a playlist of {len}");
            }
            assert_eq!(i, 0, "a lap of {len} tracks did not return to the start");
        }
    }

    // --- the repeat-one-song bug ------------------------------------------

    /// The regression this whole refactor exists for.
    ///
    /// raylib leaves `Music.looping = true` on a freshly loaded stream, so a
    /// custom track never stopped and this decision never came out as
    /// `Advance`: the driver sat in `Wait` forever and the folder played one
    /// song on repeat. `play_track` now clears the flag, which is what makes
    /// `playing` able to go false and this case reachable at all.
    #[test]
    fn a_custom_track_that_stops_advances_to_the_next_song() {
        assert_eq!(
            music_action(false, false, true),
            MusicAction::Advance,
            "a stopped, armed playlist track must move on, or the folder repeats one song"
        );
    }

    #[test]
    fn a_custom_track_that_is_still_playing_is_left_alone() {
        assert_eq!(music_action(false, true, true), MusicAction::Wait);
        assert_eq!(
            music_action(false, true, false),
            MusicAction::Wait,
            "playing wins over `armed`, so a freshly started track is not skipped"
        );
    }

    #[test]
    fn a_stopped_track_that_was_never_armed_starts_rather_than_advances() {
        // First play, or the restart after music is switched back on. Advancing
        // here would skip a whole song every time the player toggles music.
        assert_eq!(music_action(false, false, false), MusicAction::Start);
    }

    #[test]
    fn the_empty_playlist_only_ever_ensures_the_bundled_track_plays() {
        // The bundled track loops itself, so it is restarted whenever it is
        // not going and never counted as "finished".
        for playing in [true, false] {
            for armed in [true, false] {
                assert_eq!(
                    music_action(true, playing, armed),
                    MusicAction::EnsurePlaying
                );
            }
        }
    }

    #[test]
    fn turning_music_off_then_on_restarts_the_same_track() {
        // `keep_music_playing` disarms the track when it stops music, so the
        // restart lands on `Start` (same song) rather than `Advance` (skip).
        // Anything else means every mute/unmute burned a track.
        let action = music_action(false, false, false);
        assert_eq!(action, MusicAction::Start);
        assert_ne!(action, MusicAction::Advance);
    }

    /// The decision has to be total over every combination the driver can
    /// hand it, or some frame leaves the music in a state it never leaves.
    #[test]
    fn every_observation_yields_exactly_one_action() {
        for playlist_empty in [true, false] {
            for playing in [true, false] {
                for armed in [true, false] {
                    let action =
                        music_action(playlist_empty, playing, armed);
                    // An empty playlist never touches `track_armed`, so all
                    // four combinations must collapse to the same answer.
                    let expected = if playlist_empty {
                        MusicAction::EnsurePlaying
                    } else if playing {
                        MusicAction::Wait
                    } else if armed {
                        MusicAction::Advance
                    } else {
                        MusicAction::Start
                    };
                    assert_eq!(
                        action, expected,
                        "playlist_empty={playlist_empty} playing={playing} armed={armed}"
                    );
                }
            }
        }
    }
}