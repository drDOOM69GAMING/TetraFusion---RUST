//! Diagnostic for the embedded-asset loaders. Not part of the game.
//!
//! Run with `cargo run --release --example asset_probe`. It answers one
//! question: when `LoadImageFromMemory` / `LoadWaveFromMemory` refuse a file,
//! is it because the memory route is missing from this raylib build, or because
//! the data is wrong? It prints both answers by trying the same bytes from
//! memory and from a temp file on disk.

use std::io::Write;

use raylib::audio::RaylibAudio;
use raylib::core::texture::Image;

/// The same files the game embeds. `include_bytes!` here as well as in
/// `src/assets.rs` is deliberate: if the game's copy were somehow wrong, this
/// probe would still be measuring the real file.
const JPG: &[u8] = include_bytes!("../assets/backgrounds/1.jpg");
const JPG2: &[u8] = include_bytes!("../assets/backgrounds/2.jpg");
const OGG: &[u8] = include_bytes!("../assets/Lineclear.ogg");

/// Kept in step with `src/assets.rs` by hand: the example cannot import the
/// binary crate's modules. `cargo test` compiles this file, so a typo here is a
/// build error rather than a silent difference.
const IMAGE_FORMAT: &str = ".jpg";
const AUDIO_FORMAT: &str = ".ogg";

fn to_temp(tag: &str, bytes: &[u8]) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("tf_probe_{tag}"));
    let mut f = std::fs::File::create(&p).expect("create temp");
    f.write_all(bytes).expect("write temp");
    p
}

fn main() {
    let (mut rl, thread) = raylib::init()
        .size(64, 64)
        .title("asset probe")
        .build();

    println!("--- backgrounds ---");
    for (n, bytes) in [(1u32, JPG), (2, JPG2)] {
        let path = to_temp(&format!("bg{n}.jpg"), bytes);
        print!("  {n}.jpg ({} bytes)\n", bytes.len());
        match Image::load_image_from_mem(IMAGE_FORMAT, bytes) {
            Ok(img) => {
                let t = rl.load_texture_from_image(&thread, &img);
                println!(
                    "    from memory: decoded {}x{}, texture ok = {}",
                    img.width(),
                    img.height(),
                    t.is_ok()
                );
            }
            Err(e) => println!("    from memory: FAILED: {e}"),
        }
        match rl.load_texture(&thread, &path.to_string_lossy()) {
            Ok(_) => println!("    from file:   decoded"),
            Err(e) => println!("    from file:   FAILED: {e}"),
        }
        let _ = std::fs::remove_file(&path);
    }

    println!("--- audio ---");
    match RaylibAudio::init_audio_device() {
        Ok(dev) => {
            let path = to_temp("sfx.ogg", OGG);
            for (label, bytes) in [("memory ", &OGG[..]), ("file   ", &std::fs::read(&path).unwrap_or_default())] {
                let slice: &[u8] = bytes;
                match dev.new_wave_from_memory(AUDIO_FORMAT, slice) {
                    Ok(w) => println!("    from {label}: OK ({} Hz, {} ch)", w.sample_rate(), w.channels()),
                    Err(e) => println!("    from {label}: FAILED: {e}"),
                }
            }
            match dev.new_sound(&path.to_string_lossy()) {
                Ok(_) => println!("    LoadSound(file): OK"),
                Err(e) => println!("    LoadSound(file): FAILED: {e}"),
            }
            let _ = std::fs::remove_file(&path);
        }
        Err(e) => println!("  audio device unavailable: {e:?}"),
    }

    // The window closes when the handle drops.
}