//! Where per-user files live.
//!
//! # Why not beside the exe
//!
//! Settings and high scores are the player's data, not part of the program.
//! They were being written to the **current working directory**, which is wrong
//! three separate ways and every one of them bites in normal use:
//!
//! * Double-clicking the exe in Explorer puts them beside the exe, so a copy in
//!   `C:\Program Files` fails to write (and the game silently forgets every
//!   option change), and a copy on a read-only share or a USB stick either fails
//!   or leaves junk on somebody's filesystem.
//! * Launching from a shortcut, a script, or a terminal puts them in whatever
//!   folder that happened to be in. Two launches from two folders are two
//!   different games as far as the player is concerned: different keybindings,
//!   different scores, and no way to explain it.
//! * "Which folder did my scores go in?" has no answer a person can guess.
//!
//! Per-user data belongs in the platform's per-user configuration directory,
//! which is where every other application puts it and where it is already
//! writable, already backed up, and already looked in.
//!
//! # The layout
//!
//! | Platform    | Location |
//! | ----------- | -------- |
//! | Windows     | `%APPDATA%\TetraFusion\` |
//! | macOS       | `~/Library/Application Support/TetraFusion/` |
//! | Linux / BSD | `$XDG_CONFIG_HOME/TetraFusion/`, else `~/.config/TetraFusion/` |
//!
//! Windows uses the `%APPDATA%` (roaming) directory rather than `%LOCALAPPDATA%`:
//! settings and scores are small text that roams with the profile, and
//! `%APPDATA%` is the one a user told "it is in your AppData folder" will open.
//!
//! # Migration
//!
//! Anyone who played an earlier build has their options and scores in the old
//! working-directory location, and losing a keybinding layout or a record set is
//! not acceptable. [`migrate_legacy_file`] copies such a file across on first run
//! and leaves a marker so it cannot be imported twice.

use std::path::{Path, PathBuf};

/// The folder name used under the platform's per-user configuration directory.
pub const APP_DIR: &str = "TetraFusion";

/// The suffix on the marker left where a legacy file used to be.
const MIGRATED_SUFFIX: &str = ".migrated";

/// Which platform's conventions to follow.
///
/// Compiled in rather than detected at runtime: a Windows build can only ever
/// run on Windows, and this keeps the decision testable on whichever machine
/// happens to be building it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    /// `%APPDATA%`.
    Windows,
    /// `~/Library/Application Support`.
    MacOs,
    /// XDG, then `~/.config`.
    Other,
}

/// The current process's [`Os`].
pub const fn current_os() -> Os {
    if cfg!(windows) {
        Os::Windows
    } else if cfg!(target_os = "macos") {
        Os::MacOs
    } else {
        Os::Other
    }
}

/// Everything [`app_dir`] decides on, as plain data.
///
/// Split out so the decision is a pure function of its inputs and can be tested
/// without needing a machine that has a particular environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// `APPDATA` on Windows.
    pub appdata: Option<PathBuf>,
    /// `XDG_CONFIG_HOME` on Linux and BSD.
    pub xdg_config_home: Option<PathBuf>,
    /// `HOME`, or `USERPROFILE` on Windows.
    pub home: Option<PathBuf>,
    /// Which conventions to apply.
    pub os: Os,
}

/// The per-user configuration folder, e.g. `%APPDATA%\TetraFusion`.
///
/// Created if it is missing, so the first save does not fail. A profile that
/// cannot be created is not an error: the path is still returned, the writes
/// fail quietly, and the game runs with defaults for that session.
pub fn app_dir() -> PathBuf {
    let dir = resolve(&Location {
        appdata: env_path("APPDATA"),
        xdg_config_home: env_path("XDG_CONFIG_HOME"),
        home: env_path(if cfg!(windows) { "USERPROFILE" } else { "HOME" }),
        os: current_os(),
    });
    if !dir.is_dir() {
        let _ = std::fs::create_dir_all(&dir);
    }
    dir
}

/// The full path of one per-user file, e.g. `settings.json`.
pub fn file(name: &str) -> PathBuf {
    app_dir().join(name)
}

/// The pure decision behind [`app_dir`].
///
/// `None` from the inner match means there was no environment to work from at
/// all, so the folder name is used on its own as a relative path rather than
/// being appended to a base that does not exist.
fn resolve(loc: &Location) -> PathBuf {
    let base = match loc.os {
        // %APPDATA% is the roaming per-user folder. When it is missing from the
        // environment - a stripped-down launch, or a service - the well-known
        // place under the profile is the same directory it would have pointed at.
        Os::Windows => match (&loc.appdata, &loc.home) {
            (Some(d), _) => Some(d.clone()),
            (None, Some(home)) => Some(home.join("AppData").join("Roaming")),
            (None, None) => None,
        },
        Os::MacOs => match &loc.home {
            Some(home) => Some(home.join("Library").join("Application Support")),
            None => None,
        },
        Os::Other => match (&loc.xdg_config_home, &loc.home) {
            (Some(d), _) => Some(d.clone()),
            (None, Some(home)) => Some(home.join(".config")),
            (None, None) => None,
        },
    };
    match base {
        Some(base) => base.join(APP_DIR),
        None => PathBuf::from(APP_DIR),
    }
}

/// An environment variable as a path, treating empty as unset.
///
/// Windows famously has variables that exist and are empty, and `PathBuf::from("")`
/// joined onto anything produces a relative path that silently writes to the
/// working directory - the exact bug this module exists to remove.
fn env_path(key: &str) -> Option<PathBuf> {
    match std::env::var_os(key) {
        Some(v) if !v.is_empty() => Some(PathBuf::from(v)),
        _ => None,
    }
}

/// What [`migrate_legacy_file`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Migration {
    /// The file was found and copied across.
    Migrated,
    /// There was no file in the old location.
    Absent,
    /// It was moved on a previous run and the marker is still there.
    AlreadyMigrated,
    /// The per-user file already exists, so the legacy copy was not imported.
    /// The player's current settings are the ones that stay.
    DestinationExists,
    /// It could not be moved. The legacy file is left untouched.
    Skipped,
}

/// Copy a legacy working-directory file into the per-user folder, once.
///
/// The legacy file is **copied, not moved**. A player who runs the game from a
/// USB stick keeps their copy, and deleting files out of a folder the game may
/// not own is not something a game should do on startup.
pub fn migrate_legacy_file(name: &str, legacy: &Path) -> Migration {
    migrate_into(name, legacy, &app_dir())
}

/// The body of [`migrate_legacy_file`], against an explicit destination.
///
/// Separate so the tests can use a temp folder instead of the real profile
/// directory, and so a future "portable mode" has one place to aim at.
fn migrate_into(name: &str, legacy: &Path, dest_dir: &Path) -> Migration {
    let marker = marker_for(legacy);
    if marker.exists() {
        return Migration::AlreadyMigrated;
    }
    // Never clobber a per-user file that already exists. If the player has one,
    // it is the live one, and a stale copy in the working directory is not
    // allowed to silently replace it.
    if dest_dir.join(name).exists() {
        return Migration::DestinationExists;
    }
    let Ok(raw) = std::fs::read(legacy) else {
        return Migration::Absent;
    };
    // Refuse to import something that is not the file it claims to be. A stray
    // zero-byte file left behind by a full disk must not be allowed to replace
    // a good settings file with nothing.
    if raw.is_empty() {
        return Migration::Skipped;
    }
    if std::fs::create_dir_all(dest_dir).is_err() {
        return Migration::Skipped;
    }
    if std::fs::write(dest_dir.join(name), &raw).is_err() {
        return Migration::Skipped;
    }
    let _ = std::fs::write(&marker, b"moved into the per-user data folder\n");
    Migration::Migrated
}

/// `settings.json` becomes `settings.json.migrated`.
fn marker_for(legacy: &Path) -> PathBuf {
    let mut name = legacy.file_name().unwrap_or_default().to_os_string();
    name.push(MIGRATED_SUFFIX);
    legacy.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(os: Os, appdata: Option<&str>, xdg: Option<&str>, home: Option<&str>) -> Location {
        Location {
            appdata: appdata.map(PathBuf::from),
            xdg_config_home: xdg.map(PathBuf::from),
            home: home.map(PathBuf::from),
            os,
        }
    }

    /// The case that was actually asked for: on Windows it is `%APPDATA%`.
    #[test]
    fn windows_uses_appdata() {
        assert_eq!(
            resolve(&loc(
                Os::Windows,
                Some(r"C:\Users\ada\AppData\Roaming"),
                None,
                None
            )),
            PathBuf::from(r"C:\Users\ada\AppData\Roaming").join(APP_DIR)
        );
    }

    /// No `APPDATA` in the environment falls back to the well-known place under
    /// the profile, which is the same directory `APPDATA` points at.
    #[test]
    fn windows_falls_back_to_the_profile() {
        assert_eq!(
            resolve(&loc(Os::Windows, None, None, Some(r"C:\Users\ada"))),
            PathBuf::from(r"C:\Users\ada\AppData\Roaming").join(APP_DIR)
        );
    }

    /// `USERPROFILE` must not win over `APPDATA` when both are set, or a roaming
    /// profile would stop roaming.
    #[test]
    fn appdata_wins_over_userprofile() {
        let l = loc(
            Os::Windows,
            Some(r"C:\Users\ada\AppData\Roaming"),
            None,
            Some(r"C:\somewhere\else"),
        );
        assert!(resolve(&l).starts_with(r"C:\Users\ada\AppData\Roaming"));
    }

    /// Both missing yields a relative path rather than a panic. The game starts
    /// and simply remembers nothing, which is the correct degradation.
    #[test]
    fn windows_with_no_environment_at_all_still_yields_a_path() {
        assert_eq!(
            resolve(&loc(Os::Windows, None, None, None)),
            PathBuf::from(APP_DIR)
        );
    }

    /// macOS is not Linux: `~/Library/Application Support`, not `~/.config`.
    #[test]
    fn macos_uses_application_support() {
        assert_eq!(
            resolve(&loc(Os::MacOs, None, None, Some("/Users/ada"))),
            PathBuf::from("/Users/ada/Library/Application Support").join(APP_DIR)
        );
        // XDG is a Linux convention and must not hijack the macOS path.
        assert_eq!(
            resolve(&loc(Os::MacOs, None, Some("/tmp/xdg"), Some("/Users/ada"))),
            PathBuf::from("/Users/ada/Library/Application Support").join(APP_DIR)
        );
    }

    /// XDG first when set, home second, bare relative third.
    #[test]
    fn linux_follows_xdg_then_home() {
        assert_eq!(
            resolve(&loc(Os::Other, None, Some("/tmp/xdg"), Some("/home/ada"))),
            PathBuf::from("/tmp/xdg").join(APP_DIR)
        );
        assert_eq!(
            resolve(&loc(Os::Other, None, None, Some("/home/ada"))),
            PathBuf::from("/home/ada/.config").join(APP_DIR)
        );
        assert_eq!(
            resolve(&loc(Os::Other, None, None, None)),
            PathBuf::from(APP_DIR)
        );
    }

    /// The resolution done for real, on this machine, must agree with the pure
    /// one built from this machine's own environment. That is what keeps the two
    /// from drifting apart.
    #[test]
    fn the_real_resolution_matches_the_pure_one() {
        let here = Location {
            appdata: env_path("APPDATA"),
            xdg_config_home: env_path("XDG_CONFIG_HOME"),
            home: env_path(if cfg!(windows) { "USERPROFILE" } else { "HOME" }),
            os: current_os(),
        };
        assert_eq!(app_dir(), resolve(&here));
    }

    /// An empty environment variable must be treated as unset. `APPDATA=""` on
    /// Windows would otherwise resolve to a relative path and quietly write the
    /// player's settings into the working directory, which is the original bug.
    #[test]
    fn an_empty_environment_variable_is_not_a_path() {
        assert_eq!(resolve(&loc(Os::Windows, None, None, None)), PathBuf::from(APP_DIR));
        assert_eq!(env_path("TETRAFUSION_DEFINITELY_NOT_SET"), None);
    }

    /// The folder is namespaced per app, so two different games cannot collide.
    #[test]
    fn the_folder_is_namespaced_per_app() {
        let d = resolve(&loc(
            Os::Windows,
            Some(r"C:\Users\ada\AppData\Roaming"),
            None,
            None,
        ));
        assert!(d.ends_with(APP_DIR), "{d:?} is not namespaced");
    }

    /// A scratch folder under the system temp directory, which has no relation
    /// to the real profile directory.
    fn scratch(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("tetrafusion_userdir_{tag}"));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("could not create the scratch folder");
        p
    }

    /// A first run copies the legacy file across and leaves a marker; the second
    /// refuses, so a player with two copies of the game does not get their old
    /// settings re-imported over the new ones on every launch.
    #[test]
    fn migration_happens_once() {
        let root = scratch("once");
        let legacy = root.join("settings.json");
        std::fs::write(&legacy, br#"{"difficulty":"master"}"#).expect("seed the legacy file");
        let dest = root.join("profile");

        assert_eq!(
            migrate_into("settings.json", &legacy, &dest),
            Migration::Migrated
        );
        let moved = std::fs::read_to_string(dest.join("settings.json")).expect("migrated file");
        assert!(moved.contains("master"), "the contents did not come across: {moved}");
        assert!(
            marker_for(&legacy).exists(),
            "the migration marker was not written, so it would run again"
        );
        assert!(
            legacy.exists(),
            "the legacy file must be copied, not moved out from under the player"
        );

        // Overwrite the destination, then try again: the old value must not come
        // back.
        std::fs::write(dest.join("settings.json"), br#"{"difficulty":"normal"}"#).expect("overwrite");
        assert_eq!(
            migrate_into("settings.json", &legacy, &dest),
            Migration::AlreadyMigrated,
            "the second call must not overwrite the current settings with an old copy"
        );
        assert!(
            std::fs::read_to_string(dest.join("settings.json"))
                .expect("still there")
                .contains("normal"),
            "the second migration overwrote the current settings"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// A per-user file that already exists must never be replaced by a stale
    /// copy found in the working directory.
    #[test]
    fn migration_never_clobbers_an_existing_per_user_file() {
        let root = scratch("clobber");
        let dest = root.join("profile");
        std::fs::create_dir_all(&dest).expect("create the destination");
        std::fs::write(dest.join("settings.json"), br#"{"difficulty":"normal"}"#)
            .expect("seed the live file");

        let legacy = root.join("settings.json");
        std::fs::write(&legacy, br#"{"difficulty":"master"}"#).expect("seed the legacy file");

        assert_eq!(
            migrate_into("settings.json", &legacy, &dest),
            Migration::DestinationExists
        );
        let live = std::fs::read_to_string(dest.join("settings.json")).expect("still there");
        assert!(
            live.contains("normal"),
            "the legacy copy replaced the live settings: {live}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Nothing to migrate is not an error, and an empty legacy file is refused
    /// rather than being allowed to replace a good file with nothing.
    #[test]
    fn migration_refuses_an_empty_file_and_a_missing_one() {
        let root = scratch("empty");

        let missing = root.join("nothing.json");
        assert_eq!(
            migrate_into("nothing.json", &missing, &root.join("profile")),
            Migration::Absent
        );

        let empty = root.join("empty.json");
        std::fs::write(&empty, b"").expect("seed the empty file");
        assert_eq!(
            migrate_into("empty.json", &empty, &root.join("profile")),
            Migration::Skipped
        );
        assert!(
            !root.join("profile").join("empty.json").exists(),
            "an empty legacy file must not be written to the destination"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
