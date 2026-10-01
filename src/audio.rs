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
    use super::{music_action, next_track, MusicAction};

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