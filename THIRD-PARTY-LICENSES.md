# Third party licenses

TetraFusion (the Rust edition) is MIT licensed. See `LICENSE` for the full
text, copyright 2026 drDOOM69GAMING.

This project is a Rust port of TetraFusion 2.1, the original Pygame game by
the same author, which is also MIT licensed.

## Bundled at link time

The release executable statically links these crates, so their licenses apply
to the shipped binary.

| Component | License |
| --- | --- |
| [raylib](https://github.com/raysan5/raylib) (via `raylib-rs`) | zlib / libpng |
| [rand](https://github.com/rust-random/rand) | MIT OR Apache-2.0 |
| [serde](https://github.com/serde-rs/serde) | MIT OR Apache-2.0 |
| [serde_json](https://github.com/serde-rs/json) | MIT OR Apache-2.0 |

The `windows-sys` crate is deliberately **not** used. Its generated `user32`
bindings declare `CloseWindow`, which raylib's own C source also defines, and
the two collide at link time. `src/folderpick.rs` therefore calls the desktop
folder dialog through the platform's own command line tools instead of
through a crate.

## Art and audio

`assets/backgrounds/*.jpg`, `assets/*.ogg` and `assets/ICON1.ico` come from
the original game and remain the original author's work, under the same MIT
license. `assets/tetris-blocks.TTF` is not compiled into the executable; the
game only uses it if it happens to sit next to the binary.
