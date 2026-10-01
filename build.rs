use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Embeds `assets/ICON1.ico` into the executable's resource section.
///
/// Why a build script at all: a Windows window's icon is not a runtime
/// property. Explorer, the taskbar button, Alt-Tab and the title bar all read
/// the icon group resource compiled into the `.exe`, and `SetWindowIcon` can
/// only change the running window's HICON, which affects neither the file in
/// Explorer nor the pinned taskbar entry. So `ICON1.ico` has to be *linked in*,
/// which is what this does.
///
/// The resource is declared under two names on purpose:
///
/// * `GLFW_ICON` is the exact string raylib's GLFW backend looks for when it
///   registers the window class (`LoadImageW(hModule, L"GLFW_ICON", ...)`), so
///   the title bar and taskbar button pick it up with no runtime code.
/// * `1` is the conventional numeric group icon that the shell's icon handler
///   looks for, which is what makes Explorer show it on the file itself.
///
/// Declaring both costs one extra copy of the ~11 KB icon and removes the need
/// to guess which lookup the shell happens to use.
///
/// Every failure is non-fatal by design. If no resource compiler is installed
/// the game still builds and still runs â€” it just carries the default icon.
/// Refusing to compile over a decorative file would be the wrong trade, so the
/// missing-tool paths return quietly and only report under
/// `TETRAFUSION_RC_TRACE=1`.
fn main() {
    println!("cargo:rerun-if-changed=assets/ICON1.ico");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=TETRAFUSION_RC_TRACE");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let icon = manifest.join("assets").join("ICON1.ico");

    let trace = env::var_os("TETRAFUSION_RC_TRACE").is_some();
    if !icon.exists() {
        if trace {
            eprintln!("icon: {} is missing, leaving the default in place", icon.display());
        }
        return;
    }

    let msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let script = out_dir.join("tetrafusion.rc");
    // Resource compilers resolve relative icon paths against the current
    // directory, not against the script, and a Windows path written into an
    // `.rc` file is safest with forward slashes â€” `rc.exe` treats a lone
    // backslash as the start of an escape.
    let quoted = icon.to_string_lossy().replace('\\', "/");
    if let Err(err) = std::fs::write(
        &script,
        format!("1 ICON \"{quoted}\"\nGLFW_ICON ICON \"{quoted}\"\n"),
    ) {
        if trace {
            eprintln!("icon: could not write {}: {err}", script.display());
        }
        return;
    }

    let Some(tool) = find_resource_compiler(msvc) else {
        if trace {
            eprintln!(
                "icon: no resource compiler found (looked for {}); \
                 leaving the default in place",
                if msvc { "llvm-rc/rc" } else { "windres" }
            );
        }
        return;
    };

    let compiled = out_dir.join(if msvc { "tetrafusion.res" } else { "tetrafusion.o" });
    let status = match tool.name {
        // windres is GNU and takes -i/-o; llvm-rc and rc.exe are the same
        // dialect and take /fo plus an input path.
        "windres" => Command::new(&tool.path)
            .arg("-i")
            .arg(&script)
            .arg("-o")
            .arg(&compiled)
            .status(),
        _ => Command::new(&tool.path)
            .arg("/fo")
            .arg(&compiled)
            .arg(&script)
            .status(),
    };

    match status {
        Ok(status) if status.success() && compiled.exists() => {
            // link-arg rather than link-lib: the resource is an ordinary
            // linker input file, not a library to search.
            println!("cargo:rustc-link-arg-bins={}", compiled.display());
            if trace {
                eprintln!(
                    "icon: embedded {} as resource {} (via {})",
                    icon.display(),
                    compiled.display(),
                    tool.path.display()
                );
            }
        }
        Ok(status) => {
            if trace {
                eprintln!("icon: {} failed with {status}", tool.path.display());
            }
        }
        Err(err) => {
            if trace {
                eprintln!("icon: could not run {}: {err}", tool.path.display());
            }
        }
    }
}

struct Tool {
    path: PathBuf,
    name: &'static str,
}

/// Looks for a resource compiler on `PATH`, then in the usual install spots.
///
/// `PATH` alone is not enough: a Rust toolchain installed by rustup does not
/// bring a resource compiler with it, and the LLVM that ships alongside Visual
/// Studio is frequently absent from `PATH` even when it is installed. The
/// fallback list is what makes the icon appear on a machine that has LLVM but
/// never configured it.
fn find_resource_compiler(msvc: bool) -> Option<Tool> {
    let wanted: &[&str] = if msvc {
        &["llvm-rc", "rc"]
    } else {
        &["windres"]
    };
    let fallbacks: &[&str] = if msvc {
        &["C:\\Program Files\\LLVM\\bin", "C:\\Program Files (x86)\\LLVM\\bin"]
    } else {
        &["C:\\msys64\\mingw64\\bin", "C:\\msys64\\usr\\bin"]
    };

    if let Ok(path) = env::var("PATH") {
        for dir in env::split_paths(&path) {
            for name in wanted {
                let candidate = dir.join(if name.ends_with(".exe") {
                    name.to_string()
                } else {
                    format!("{name}.exe")
                });
                if candidate.is_file() {
                    return Some(Tool {
                        path: candidate,
                        name: if *name == "windres" { "windres" } else { "rc" },
                    });
                }
            }
        }
    }

    for dir in fallbacks {
        for name in wanted {
            let candidate = Path::new(dir).join(format!("{name}.exe"));
            if candidate.is_file() {
                return Some(Tool {
                    path: candidate,
                    name: if *name == "windres" { "windres" } else { "rc" },
                });
            }
        }
    }

    None
}