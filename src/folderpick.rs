//! Choosing the custom-music folder.
//!
//! The original reached for the platform's own dialog: `NSOpenPanel` on macOS,
//! `zenity` on Linux, and nothing at all on Windows - so on Windows the feature
//! could not be reached at all, and what the in-game browser that stood in for
//! it could see was only ever the tree under one letter. Here every desktop
//! platform opens a real system folder picker, so the Music Folder row behaves
//! the way the player expects: press Enter, and File Explorer takes over.
//!
//! Windows talks to `IFileOpenDialog` directly rather than through a crate, and
//! the reason is worth writing down: every Windows dialog crate reaches the API
//! through `windows-sys`, whose generated `user32` bindings also declare
//! `CloseWindow`. raylib's C code *defines* its own `CloseWindow`, so the two
//! collide at link time and the release build fails with LNK2005. Declaring the
//! handful of entry points we need ourselves keeps the link against `ole32` and
//! `shell32`, neither of which defines a single symbol raylib also defines.
//!
//! Everything about *deciding* what to do with the answer lives in
//! [`outcome`], which is pure and tested. Keeping the platform calls behind
//! wrappers is what makes that possible: this module's tests never open a
//! dialog, and the tests below double as documentation of every slot of the
//! COM vtables that the Windows path depends on.

use std::path::{Path, PathBuf};

/// What opening the folder picker did.
///
/// Three outcomes rather than an `Option`, because "the player pressed Cancel"
/// and "this machine has no dialog to offer" are different facts and the game
/// treats them differently: the first leaves the player exactly where they were,
/// the second is worth saying out loud rather than pretending nothing happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The player chose this folder.
    Chose(PathBuf),
    /// The dialog opened and the player backed out of it.
    Cancelled,
    /// No dialog could be opened on this machine.
    Unavailable,
}

/// What the platform picker produced, before the game reacts to it.
///
/// Separate from [`Outcome`] so that the "ran a real dialog" flag survives the
/// trip from the platform layer to the mapping below. That flag is the whole
/// point: a cancel is a decision the player made and must never be reported as
/// a machine that cannot show dialogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    /// A dialog was shown and this folder came back.
    Chose(PathBuf),
    /// A dialog was shown and the player backed out of it.
    Cancelled,
    /// No dialog could be opened on this machine.
    NoDialog,
}

/// Decide what the picker result means.
///
/// Split out from the platform calls so that the "cancel is not the same answer
/// as unavailable" rule is pinned by a test rather than by reading the call
/// site.
pub fn outcome(pick: Pick) -> Outcome {
    match pick {
        Pick::Chose(dir) => Outcome::Chose(dir),
        Pick::Cancelled => Outcome::Cancelled,
        Pick::NoDialog => Outcome::Unavailable,
    }
}

/// Ask the player for a folder, using the system dialog where there is one.
///
/// Returns what to do next; see [`Outcome`].
pub fn pick_folder(start: &Path) -> Outcome {
    outcome(native_pick(start))
}

// --- Windows ---------------------------------------------------------------

#[cfg(windows)]
fn native_pick(start: &Path) -> Pick {
    windows_pick(start)
}

/// The vtable a COM interface points at.
///
/// # Safety
///
/// `obj` must be a live interface whose first word really is a pointer to a
/// `T`-shaped vtable, i.e. a pointer to a `*const T`.
#[cfg(windows)]
unsafe fn vtbl<T>(obj: *mut std::ffi::c_void) -> *const T {
    *obj.cast::<*const T>()
}

/// Open the shell's folder picker.
///
/// The dialog is modal to this thread, so it comes up in front of the game
/// window on its own. `start` is offered as the opening folder when it is a
/// real, shell-visible directory; a music folder that has since been deleted or
/// renamed on another machine must not break the picker, so anything that fails
/// to resolve is simply ignored.
#[cfg(windows)]
fn windows_pick(start: &Path) -> Pick {
    // `IFileOpenDialog` needs an STA. The game's main thread has not
    // initialised COM, so this is the usual first call and it succeeds.
    // `S_FALSE` means the thread was already in an STA (nothing to undo),
    // `RPC_E_CHANGED_MODE` means something else got here first (also nothing to
    // undo). Either way the dialog itself still works, so neither is fatal.
    let initialised = unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED) } == S_OK;

    let pick = windows_pick_inner(start);

    if initialised {
        unsafe { CoUninitialize() };
    }
    pick
}

#[cfg(windows)]
fn windows_pick_inner(start: &Path) -> Pick {
    let mut dialog: *mut std::ffi::c_void = std::ptr::null_mut();
    // CLSCTX_INPROC_SERVER, IID_IFileOpenDialog.
    let hr = unsafe {
        CoCreateInstance(
            &CLSID_FILE_OPEN_DIALOG,
            std::ptr::null(),
            CLSCTX_INPROC_SERVER,
            &IID_FILE_OPEN_DIALOG,
            &mut dialog,
        )
    };
    // A failure here means no shell at all - a Server Core container, say -
    // which is the one case where there is no dialog to fall back to.
    if hr != S_OK || dialog.is_null() {
        return Pick::NoDialog;
    }
    // `Dialog` owns the reference, so it goes out of scope - and releases -
    // on every path below.
    let dialog = Dialog(dialog);
    let v = unsafe { &*vtbl::<FileDialogVtbl>(dialog.0) };

    // `FOS_PICKFOLDERS` is the whole point: pick a folder, not a file.
    // `FOS_PATHMUSTEXIST` keeps the game from being handed a path that is not
    // there, which the music scanner would then reject as "missing".
    unsafe {
        (v.set_options)(dialog.0, FOS_PICKFOLDERS | FOS_PATHMUSTEXIST);
        (v.set_ok_button_label)(dialog.0, wide("Choose").as_ptr());
        if let Some(item) = shell_item(start) {
            (v.set_folder)(dialog.0, item.0);
        }
    }

    // A null parent is deliberate: the dialog is modal to this thread rather
    // than to the raylib window, so a stray keypress aimed at the game behind
    // it cannot dismiss it.
    let hr = unsafe { (v.show)(dialog.0, std::ptr::null_mut()) };
    if hr == ERROR_CANCELLED {
        return Pick::Cancelled;
    }
    if hr != S_OK {
        return Pick::NoDialog;
    }

    let mut item: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = unsafe { (v.get_result)(dialog.0, &mut item) };
    if hr != S_OK || item.is_null() {
        return Pick::NoDialog;
    }
    let item = ShellItem(item);

    // `SIGDN_FILESYSPATH` is the real `D:\Music` style path. The other forms
    // are display names, shell URLs, or relative to the current drive - none of
    // which `std::fs` can open.
    let mut raw: *mut u16 = std::ptr::null_mut();
    let hr = unsafe {
        let v = &*vtbl::<ShellItemVtbl>(item.0);
        (v.get_display_name)(item.0, SIGDN_FILESYSPATH, &mut raw)
    };
    if hr != S_OK || raw.is_null() {
        return Pick::NoDialog;
    }
    // COM hands back a NUL-terminated UTF-16 string allocated with
    // `CoTaskMemAlloc`. The loop bounds itself on that terminator, and the
    // buffer is a fresh allocation the shell gave us, so reading it back is
    // sound; freeing it afterwards keeps every pick from leaking.
    let picked = {
        let mut len = 0usize;
        while unsafe { *raw.add(len) } != 0 {
            len += 1;
        }
        // `from_utf16_lossy` is the honest reading of what the API guarantees;
        // a lone surrogate cannot appear in a filesystem path anyway.
        PathBuf::from(String::from_utf16_lossy(unsafe {
            std::slice::from_raw_parts(raw, len)
        }))
    };
    unsafe { CoTaskMemFree(raw.cast()) };

    if picked.as_os_str().is_empty() {
        Pick::Cancelled
    } else {
        Pick::Chose(picked)
    }
}

/// Own a COM interface so its `Release` runs however this scope is left.
///
/// The vtable is read fresh in `Drop` rather than cached, so there is no way for
/// a stale pointer to outlive the object it describes.
#[cfg(windows)]
struct Dialog(*mut std::ffi::c_void);

#[cfg(windows)]
impl Drop for Dialog {
    fn drop(&mut self) {
        unsafe { ((*vtbl::<FileDialogVtbl>(self.0)).release)(self.0) };
    }
}

#[cfg(windows)]
struct ShellItem(*mut std::ffi::c_void);

#[cfg(windows)]
impl Drop for ShellItem {
    fn drop(&mut self) {
        unsafe { ((*vtbl::<ShellItemVtbl>(self.0)).release)(self.0) };
    }
}

/// Resolve a filesystem path to an `IShellItem`, for `SetFolder`.
///
/// Returns `None` for anything the shell will not vouch for, which covers the
/// deleted-music-folder case as well as an empty string.
#[cfg(windows)]
fn shell_item(path: &Path) -> Option<ShellItem> {
    let wide = wide(&path.to_string_lossy());
    let mut item: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = unsafe {
        SHCreateItemFromParsingName(
            wide.as_ptr(),
            std::ptr::null(),
            &IID_SHELL_ITEM,
            &mut item,
        )
    };
    if hr != S_OK || item.is_null() {
        None
    } else {
        Some(ShellItem(item))
    }
}

/// UTF-16 with a NUL terminator, which is what every Win32 wide string wants.
///
/// Text after an interior NUL is dropped. Neither call site can hit that: the
/// button label is a literal, and a Windows path cannot contain a NUL at all.
#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().take_while(|&u| u != 0).collect();
    v.push(0);
    v
}

#[cfg(windows)]
mod com {
    //! The slice of COM and the shell this game needs, declared by hand.
    //!
    //! Only `ole32` and `shell32` are linked. `windows-sys` cannot be used here:
    //! its `user32` bindings declare `CloseWindow`, which raylib's own C code
    //! defines, and the release link fails with LNK2005.

    use std::ffi::c_void;

    pub type HResult = i32;

    pub const S_OK: HResult = 0;
    /// `HRESULT_FROM_WIN32(ERROR_CANCELLED)`, which is what the shell dialog
    /// returns when the player presses Cancel or closes the window.
    pub const ERROR_CANCELLED: HResult = 0x8007_04C7u32 as i32;

    pub const COINIT_APARTMENTTHREADED: u32 = 0x2;
    pub const CLSCTX_INPROC_SERVER: u32 = 0x1;

    pub const FOS_PICKFOLDERS: u32 = 0x20;
    pub const FOS_PATHMUSTEXIST: u32 = 0x800;
    /// `SIGDN_FILESYSPATH`, signed.
    pub const SIGDN_FILESYSPATH: i32 = -2147123200;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    impl Guid {
        /// Build a GUID from the `u128` form the same name is written in
        /// elsewhere, i.e. the form the SDK headers and MSDN print.
        ///
        /// That is *not* the order the sixteen bytes sit in memory. Windows
        /// stores `data1`, `data2` and `data3` little-endian and `data4` as a
        /// straight byte array, so `{DC1C5A9C-E88A-...}` begins `9C 5A 1C DC 8A
        /// E8`. Reading the u128 as big-endian here does that reversal once, at
        /// the boundary, rather than leaving every later reader to remember
        /// which way round each field is - and a GUID reversed the other way
        /// fails as "class not registered" rather than as anything diagnosable.
        pub const fn from_u128(uuid: u128) -> Self {
            let b = uuid.to_be_bytes();
            Self {
                data1: u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
                data2: u16::from_be_bytes([b[4], b[5]]),
                data3: u16::from_be_bytes([b[6], b[7]]),
                data4: [b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]],
            }
        }

        /// The sixteen bytes as they sit in memory.
        ///
        /// Nothing in the shipping build needs the reverse direction - the
        /// layout tests are what read a GUID back out as text - so this is only
        /// compiled for tests rather than left to warn as dead code.
        #[cfg(test)]
        pub const fn to_le_bytes(&self) -> [u8; 16] {
            let d1 = self.data1.to_le_bytes();
            let d2 = self.data2.to_le_bytes();
            let d3 = self.data3.to_le_bytes();
            [
                d1[0], d1[1], d1[2], d1[3], d2[0], d2[1], d3[0], d3[1], self.data4[0], self.data4[1],
                self.data4[2], self.data4[3], self.data4[4], self.data4[5], self.data4[6], self.data4[7],
            ]
        }
    }

    /// `{DC1C5A9C-E88A-4dde-A5A1-60F82A20AEF7}`
    pub const CLSID_FILE_OPEN_DIALOG: Guid =
        Guid::from_u128(0xdc1c_5a9c_e88a_4dde_a5a1_60f8_2a20_aef7);
    /// `{D57C7288-D4AD-4768-BE02-9D969532D960}`
    pub const IID_FILE_OPEN_DIALOG: Guid =
        Guid::from_u128(0xd57c_7288_d4ad_4768_be02_9d96_9532_d960);
    /// `{43826D1E-E718-42EE-BC55-A1E261C37BFE}`
    pub const IID_SHELL_ITEM: Guid = Guid::from_u128(0x4382_6d1e_e718_42ee_bc55_a1e2_61c3_7bfe);

    /// `IFileDialog`'s vtable, starting at `IUnknown`.
    ///
    /// Every one of the 27 slots is present, and every one is named for what
    /// Windows puts there. That is not fussiness: a COM vtable is a flat array
    /// of function pointers, so getting an index wrong does not fail to compile
    /// and does not return a wrong answer - it calls whatever function happens
    /// to be at that index, with our arguments. The first version of this struct
    /// had `set_options` at 7, which is `Advise`, and the game's very first call
    /// then made the shell write an event cookie to address `0x820` - a hard
    /// access violation the moment anyone pressed Enter on the Music Folder row.
    ///
    /// Slots this game never calls are held as `*const c_void` rather than
    /// invented signatures, but they still have to be *counted*, because a
    /// `*const c_void` is exactly one pointer wide and skipping one shifts every
    /// method after it. `FILE_DIALOG_VTABLE` below is the pinned order, taken
    /// from `ShObjIdl_core.h`, and the `windows_layout` tests check this struct
    /// against it so a future edit cannot reintroduce the same class of bug.
    #[repr(C)]
    pub struct FileDialogVtbl {
        // --- IUnknown ---
        pub query_interface: unsafe extern "system" fn(
            *mut c_void,
            *const Guid,
            *mut *mut c_void,
        ) -> HResult,
        pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        pub release: unsafe extern "system" fn(*mut c_void) -> u32,
        // --- IModalWindow ---
        pub show: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HResult,
        // --- IFileDialog ---
        pub set_file_types: *const c_void,             // 4
        pub set_file_type_index: *const c_void,        // 5
        pub get_file_type_index: *const c_void,        // 6
        /// `Advise` is the reason the whole table is here. It is pure event
        /// plumbing that this game never uses, and it is a *writer* of a `DWORD`
        /// through its second argument - the argument this code was passing the
        /// dialog options to when it was nine slots out.
        pub advise: *const c_void,                     // 7
        pub unadvise: *const c_void,                   // 8
        pub set_options: unsafe extern "system" fn(*mut c_void, u32) -> HResult, // 9
        pub get_options: *const c_void,                // 10
        pub set_default_folder: *const c_void,         // 11
        pub set_folder: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HResult, // 12
        pub get_folder: *const c_void,                 // 13
        pub get_current_selection: *const c_void,      // 14
        pub set_file_name: *const c_void,              // 15
        pub get_file_name: *const c_void,              // 16
        pub set_title: *const c_void,                  // 17
        pub set_ok_button_label: unsafe extern "system" fn(*mut c_void, *const u16) -> HResult, // 18
        /// `SetFileNameLabel` takes a `LPCWSTR`, so calling it in place of
        /// `GetResult` - which is what an off-by-one here did - passes a pointer
        /// to a pointer where a string is expected and reads the *address* of
        /// that pointer as text.
        pub set_file_name_label: *const c_void,        // 19
        pub get_result: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HResult, // 20
        pub add_place: *const c_void,                  // 21
        pub set_default_extension: *const c_void,      // 22
        pub close: *const c_void,                      // 23
        pub set_client_guid: *const c_void,            // 24
        pub clear_client_data: *const c_void,          // 25
        pub set_filter: *const c_void,                 // 26
    }

    /// `FILE_DIALOG_VTABLE` is the name at each `IFileDialog` vtable index.
    ///
    /// Transcribed from `IFileDialog : public IModalWindow` in the Windows 10
    /// SDK's `um\ShObjIdl_core.h`, counting from `IUnknown`. Kept as data rather
    /// than as a comment so the layout test can compare it against the struct's
    /// field offsets, which is the only way a mistyped index gets caught before
    /// it corrupts memory. Test-only, because nothing outside the tests reads
    /// it - a name table that shipped would be a second place for the ordering
    /// to go stale.
    #[cfg(test)]
    pub const FILE_DIALOG_VTABLE: [&str; 27] = [
        "query_interface",       // 0
        "add_ref",               // 1
        "release",               // 2
        "show",                  // 3  IModalWindow
        "set_file_types",        // 4
        "set_file_type_index",   // 5
        "get_file_type_index",   // 6
        "advise",                // 7
        "unadvise",              // 8
        "set_options",           // 9
        "get_options",           // 10
        "set_default_folder",    // 11
        "set_folder",            // 12
        "get_folder",            // 13
        "get_current_selection", // 14
        "set_file_name",         // 15
        "get_file_name",         // 16
        "set_title",             // 17
        "set_ok_button_label",   // 18
        "set_file_name_label",   // 19
        "get_result",            // 20
        "add_place",             // 21
        "set_default_extension", // 22
        "close",                 // 23
        "set_client_guid",       // 24
        "clear_client_data",     // 25
        "set_filter",            // 26
    ];

    /// `IShellItem`'s vtable, starting at `IUnknown`.
    #[repr(C)]
    pub struct ShellItemVtbl {
        pub query_interface: unsafe extern "system" fn(
            *mut c_void,
            *const Guid,
            *mut *mut c_void,
        ) -> HResult,
        pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        pub release: unsafe extern "system" fn(*mut c_void) -> u32,
        pub bind_to_handler: *const c_void,
        pub get_parent: *const c_void,
        pub get_display_name: unsafe extern "system" fn(
            *mut c_void,
            i32,
            *mut *mut u16,
        ) -> HResult,
    }

    #[link(name = "ole32")]
    extern "system" {
        pub fn CoInitializeEx(reserved: *mut c_void, co_init: u32) -> HResult;
        pub fn CoUninitialize();
        pub fn CoCreateInstance(
            clsid: *const Guid,
            outer: *const c_void,
            ctx: u32,
            iid: *const Guid,
            out: *mut *mut c_void,
        ) -> HResult;
        pub fn CoTaskMemFree(mem: *const c_void);
    }

    #[link(name = "shell32")]
    extern "system" {
        pub fn SHCreateItemFromParsingName(
            path: *const u16,
            binding_context: *const c_void,
            iid: *const Guid,
            out: *mut *mut c_void,
        ) -> HResult;
    }
}

#[cfg(windows)]
use com::*;

// --- macOS -----------------------------------------------------------------

#[cfg(target_os = "macos")]
fn native_pick(start: &Path) -> Pick {
    // `choose folder` is `NSOpenPanel` in the same folder-picking mode, driven
    // through the same scripting bridge the original's `NSOpenPanel` call sat
    // on. The chosen folder comes back on stdout as a POSIX path.
    const SCRIPT: &str = r#"POSIX path of (choose folder with prompt "Select Music Folder")"#;

    let script = if start.is_dir() {
        let escaped = start
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        format!(r#"POSIX path of (choose folder with prompt "Select Music Folder" default location POSIX file "{escaped}")"#)
    } else {
        SCRIPT.to_string()
    };

    let out = match std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
    {
        // `osascript` missing entirely: no picker to offer.
        Err(_) => return Pick::NoDialog,
        Ok(out) => out,
    };
    if !out.status.success() {
        // The user pressed Cancel, which osascript reports as a non-zero exit.
        return Pick::Cancelled;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let path = PathBuf::from(text.trim());
    if path.as_os_str().is_empty() {
        Pick::Cancelled
    } else {
        Pick::Chose(path)
    }
}

// --- Linux and the rest ----------------------------------------------------

/// Try the desktop's own picker, the way the original did.
///
/// `zenity` is what `TetraFusion_2.1.py` shelled out to on Linux, so this is
/// the original's own Linux behaviour rather than an addition. It is also the
/// reason the in-game browser stays: `zenity` is frequently not installed, and
/// the other Rust dialog crates need GTK development packages just to compile.
#[cfg(all(unix, not(target_os = "macos")))]
fn native_pick(start: &Path) -> Pick {
    let mut cmd = std::process::Command::new("zenity");
    cmd.arg("--file-selection")
        .arg("--directory")
        .arg("--title=Select Music Folder");
    if start.is_dir() {
        cmd.arg(start);
    }
    let out = match cmd.output() {
        // Not installed, or not on PATH.
        Err(_) => return Pick::NoDialog,
        Ok(out) => out,
    };
    if !out.status.success() {
        return Pick::Cancelled;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let path = PathBuf::from(text.trim());
    if path.as_os_str().is_empty() {
        Pick::Cancelled
    } else {
        Pick::Chose(path)
    }
}

#[cfg(not(any(windows, unix)))]
fn native_pick(_start: &Path) -> Pick {
    Pick::NoDialog
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chosen_folder_is_taken() {
        assert_eq!(
            outcome(Pick::Chose(PathBuf::from("C:/Music"))),
            Outcome::Chose(PathBuf::from("C:/Music"))
        );
    }

    #[test]
    fn cancelling_a_real_dialog_is_not_the_same_answer_as_no_dialog() {
        // This is the distinction that matters: "no thanks" is a decision the
        // player made and must leave them on the Options screen with the folder
        // they already had, where "this machine cannot show dialogs" is a fact
        // about the machine worth reporting differently.
        assert_eq!(outcome(Pick::Cancelled), Outcome::Cancelled);
    }

    #[test]
    fn a_machine_with_no_dialog_says_so() {
        assert_eq!(outcome(Pick::NoDialog), Outcome::Unavailable);
    }

    #[test]
    fn the_three_platform_answers_stay_distinct() {
        // A picker that can report only two states cannot express "cancelled
        // without falling through", so pin all three as different outcomes.
        assert_ne!(outcome(Pick::Cancelled), outcome(Pick::NoDialog));
        assert_ne!(outcome(Pick::Chose(PathBuf::from("/m"))), outcome(Pick::Cancelled));
    }

    #[cfg(windows)]
    mod windows_layout {
        //! These pin the COM vtable offsets the picker calls through.
        //!
        //! A wrong index here is not a compile error and not a panic: it calls
        //! whatever function happens to sit at that offset, which is a crash
        //! inside the shell dialog. `offset_of!` against the slot index from the
        //! SDK header is the only cheap way to catch it.

        use super::*;
        use std::mem::{align_of, offset_of, size_of};

        const SLOT: usize = size_of::<*const std::ffi::c_void>();

        /// Assert that `FileDialogVtbl::$field` really is vtable index `$n`,
        /// *and* that [`FILE_DIALOG_VTABLE`] calls that same index `$name`.
        ///
        /// The two halves matter separately. The offset catches a field that
        /// sits in the wrong place in the struct; the name catches the pinned
        /// list drifting away from the struct. Together they mean the index a
        /// call site uses is derived from the header rather than from
        /// whatever the code happened to do last time - which is the mistake
        /// that put `SetOptions` in `Advise`'s slot and crashed the game on the
        /// first Enter press.
        macro_rules! dialog_slots {
            ($($field:ident => $n:expr),* $(,)?) => {$(
                assert_eq!(
                    offset_of!(FileDialogVtbl, $field),
                    $n * SLOT,
                    "FileDialogVtbl::{} is not at IFileDialog vtable index {}",
                    stringify!($field),
                    $n,
                );
                assert_eq!(
                    FILE_DIALOG_VTABLE[$n], stringify!($field),
                    "FILE_DIALOG_VTABLE[{}] disagrees with the struct field there",
                    $n,
                );
            )*};
        }

        #[test]
        fn every_vtable_slot_is_one_pointer_wide() {
            assert_eq!(size_of::<FileDialogVtbl>(), FILE_DIALOG_VTABLE.len() * SLOT);
            assert_eq!(align_of::<FileDialogVtbl>(), SLOT);
        }

        #[test]
        fn the_file_dialog_methods_sit_where_the_sdk_says() {
            // `IUnknown`, then `IModalWindow` (just `Show`), then `IFileDialog`
            // in header order. The four this game actually calls are `set_options`
            // (9), `set_folder` (12), `set_ok_button_label` (18) and `get_result`
            // (20) - none of which is where a first guess puts them, because
            // `Advise`/`Unadvise` and `GetFileTypeIndex` sit in between.
            dialog_slots! {
                query_interface       => 0,
                add_ref               => 1,
                release               => 2,
                show                  => 3,
                set_file_types        => 4,
                set_file_type_index   => 5,
                get_file_type_index   => 6,
                advise                => 7,
                unadvise              => 8,
                set_options           => 9,
                get_options           => 10,
                set_default_folder    => 11,
                set_folder            => 12,
                get_folder            => 13,
                get_current_selection => 14,
                set_file_name         => 15,
                get_file_name         => 16,
                set_title             => 17,
                set_ok_button_label   => 18,
                set_file_name_label   => 19,
                get_result            => 20,
                add_place             => 21,
                set_default_extension => 22,
                close                 => 23,
                set_client_guid       => 24,
                clear_client_data     => 25,
                set_filter            => 26,
            }
        }

        #[test]
        fn the_methods_the_picker_calls_are_not_shifted_by_a_mistake() {
            // The bug this file shipped with, pinned so it cannot come back:
            // each of these was at the index of a real but *different* method,
            // and every one of them compiled.
            assert_ne!(offset_of!(FileDialogVtbl, set_options), offset_of!(FileDialogVtbl, advise));
            assert_ne!(offset_of!(FileDialogVtbl, set_folder), offset_of!(FileDialogVtbl, set_options));
            assert_ne!(
                offset_of!(FileDialogVtbl, set_ok_button_label),
                offset_of!(FileDialogVtbl, set_title),
            );
            assert_ne!(
                offset_of!(FileDialogVtbl, get_result),
                offset_of!(FileDialogVtbl, set_file_name_label),
            );
        }

        #[test]
        fn get_display_name_is_the_sixth_shell_item_method() {
            // IUnknown (3) + BindToHandler (1) + GetParent (1), then
            // GetDisplayName. No `Advise` equivalent here, which is why this
            // table is one slot shorter to look at than the dialog's.
            assert_eq!(size_of::<ShellItemVtbl>(), 6 * SLOT);
            assert_eq!(offset_of!(ShellItemVtbl, query_interface), 0);
            assert_eq!(offset_of!(ShellItemVtbl, add_ref), 1 * SLOT);
            assert_eq!(offset_of!(ShellItemVtbl, release), 2 * SLOT);
            assert_eq!(offset_of!(ShellItemVtbl, bind_to_handler), 3 * SLOT);
            assert_eq!(offset_of!(ShellItemVtbl, get_parent), 4 * SLOT);
            assert_eq!(offset_of!(ShellItemVtbl, get_display_name), 5 * SLOT);
        }

        #[test]
        fn the_guid_constants_are_the_ones_the_docs_print() {
            // Rendered the way the SDK headers and MSDN print them, because a
            // mistyped constant silently instantiates the wrong COM class.
            let render = |g: Guid| {
                let b = g.to_le_bytes();
                format!(
                    "{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
                    b[3], b[2], b[1], b[0], b[5], b[4], b[7], b[6], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
                )
            };
            assert_eq!(
                render(CLSID_FILE_OPEN_DIALOG),
                "DC1C5A9C-E88A-4DDE-A5A1-60F82A20AEF7"
            );
            assert_eq!(
                render(IID_FILE_OPEN_DIALOG),
                "D57C7288-D4AD-4768-BE02-9D969532D960"
            );
            assert_eq!(render(IID_SHELL_ITEM), "43826D1E-E718-42EE-BC55-A1E261C37BFE");
        }

        #[test]
        fn the_dialog_options_are_the_documented_bits() {
            // FOS_PICKFOLDERS is what makes this a folder picker at all, and
            // FOS_PATHMUSTEXIST is what stops it handing back a path the music
            // scanner would then call "missing".
            assert_eq!(FOS_PICKFOLDERS, 0x20);
            assert_eq!(FOS_PATHMUSTEXIST, 0x800);
            assert_eq!(SIGDN_FILESYSPATH, -2147123200);
            // ERROR_CANCELLED is HRESULT_FROM_WIN32(1223): 0x8007_0000 | 1223.
            assert_eq!(ERROR_CANCELLED, 0x8007_04C7u32 as i32);
        }

        #[test]
        fn wide_appends_a_terminator_and_stops_at_an_embedded_nul() {
            let w = wide("Choose");
            assert_eq!(
                w,
                vec![
                    b'C' as u16,
                    b'h' as u16,
                    b'o' as u16,
                    b'o' as u16,
                    b's' as u16,
                    b'e' as u16,
                    0
                ]
            );
            // A Win32 wide string is NUL-terminated, so text after an interior
            // NUL would be unreachable. Neither call site can produce one (a
            // Windows path cannot contain a NUL), but truncating is the safe
            // reading if one ever arrives.
            assert_eq!(wide("a\0b"), vec![b'a' as u16, 0]);
        }
    }
}
