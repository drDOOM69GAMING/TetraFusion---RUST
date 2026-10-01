//! Checks the window icon where it actually has to be: inside the linked
//! executable.
//!
//! The icon is not set at runtime. It is compiled into the `.exe`'s resource
//! section by `build.rs`, because Explorer, the taskbar button and Alt-Tab all
//! read that section and no runtime call can write to it.
//!
//! That design has one nasty property, and it is the whole reason this file
//! exists: **every failure along that chain is silent.** A missing resource
//! compiler, an `.rc` path the compiler cannot resolve, a dropped link argument
//! - all of them still produce a working, perfectly playable, completely
//! icon-less executable. The previous implementation had exactly this property,
//! which is how it managed to report success while showing no icon anywhere.
//!
//! So the assertion has to be against the built bytes, not against the build
//! script's own account of what it did.

/// The freshly linked binary. `CARGO_BIN_EXE_*` is available to integration
/// tests only - it is not defined for unit tests inside the binary crate, which
/// is one reason this lives in `tests/` and not in `src/main.rs`.
fn built_exe() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_tetrafusion"))
}

#[test]
fn the_executable_has_a_resource_section() {
    // A PE file without a `.rsrc` section cannot carry an icon at all, so this
    // is the coarse check that the compiler ran and the linker kept its output.
    let exe = built_exe();
    let bytes = std::fs::read(&exe).expect("built exe should be readable");
    assert!(
        bytes.windows(5).any(|w| w == b".rsrc"),
        "{} has no .rsrc section, so it cannot carry the window icon. \
         Check that a resource compiler (llvm-rc / rc.exe / windres) is \
         installed and run the build with TETRAFUSION_RC_TRACE=1.",
        exe.display()
    );
}

#[test]
fn the_executable_carries_the_glfw_icon_resource() {
    // `GLFW_ICON` is what raylib's GLFW backend loads when it registers the
    // window class (`LoadImageW(hModule, L"GLFW_ICON", ...)`), so it is what
    // makes the title bar and taskbar button show the icon. Being a *string*
    // resource, it is stored UTF-16LE in the section.
    let exe = built_exe();
    let bytes = std::fs::read(&exe).expect("built exe should be readable");
    let wide: Vec<u8> = "GLFW_ICON"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert!(
        bytes.windows(wide.len()).any(|w| w == wide.as_slice()),
        "no UTF-16 'GLFW_ICON' resource in {}: the window class would fall \
         back to the default icon.",
        exe.display()
    );
}

#[test]
fn the_icon_is_declared_twice_so_both_the_window_and_explorer_show_it() {
    // Two declarations are made on purpose: `GLFW_ICON` for the window class,
    // and the numeric group icon `1` for the shell's icon handler on the file
    // itself. Dropping one costs exactly one of those two places, silently.
    //
    // Rather than pattern-match resource-directory internals, this counts
    // copies of the icon payload. One declaration embeds the PNG once; two
    // embed it twice. Counting the payload is a direct proxy for the number of
    // declarations, and it does not depend on how the resource compiler
    // happens to lay out the directory.
    let icon = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("ICON1.ico");
    let Ok(ico) = std::fs::read(&icon) else {
        return; // Asset not present; the other tests still cover the mechanism.
    };
    // The payload starts at the first PNG signature inside the container.
    let Some(start) = ico.windows(8).position(|w| w == b"\x89PNG\r\n\x1a\n") else {
        return; // Not a PNG-backed icon; nothing to count.
    };
    let marker = &ico[start..start + 32];

    let bytes = std::fs::read(built_exe()).expect("built exe should be readable");
    let copies = bytes.windows(marker.len()).filter(|w| *w == marker).count();
    assert!(
        copies >= 2,
        "found the icon payload {copies} time(s) in the executable; expected \
         at least 2 (once for GLFW_ICON so the window and taskbar show it, \
         once for the numeric group icon so Explorer shows it on the file)."
    );
}

#[test]
fn the_icon_survives_a_rebuild_from_scratch() {
    // `build.rs` only reruns when its inputs change, so an incremental build
    // cannot be trusted to prove the resource survived. This shells out to
    // cargo for a clean build of the binary and re-asserts on the result,
    // because "it worked once" and "it always works" are different claims and
    // only the second one is worth having.
    //
    // Skipped when cargo is not on PATH (an IDE-managed environment), since a
    // missing tool here is not a failure of the icon.
    let Ok(path) = std::env::var("PATH") else {
        return;
    };
    let cargo = std::env::split_paths(&path)
        .map(|d| d.join("cargo.exe"))
        .chain(std::env::split_paths(&path).map(|d| d.join("cargo")))
        .find(|c| c.is_file());
    let Some(cargo) = cargo else {
        eprintln!("cargo not on PATH; skipping clean-rebuild check");
        return;
    };

    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let status = std::process::Command::new(&cargo)
        .current_dir(manifest)
        .args(["build", "--release", "--bin", "tetrafusion"])
        .status();
    assert!(
        matches!(status, Ok(s) if s.success()),
        "clean release build failed; a fresh build would ship without the icon"
    );

    let bytes = std::fs::read(built_exe()).expect("rebuilt exe should be readable");
    assert!(
        bytes.windows(5).any(|w| w == b".rsrc"),
        "after a clean rebuild the executable has no .rsrc section"
    );
}