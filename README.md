# TetraFusion 2.1  Rust Edition

A faithful Rust port of **TetraFusion 2.1** by drDOOM69GAMING (MIT-licensed
Pygame game), built with [raylib-rs](https://github.com/raysan5/raylib-rs)
(raylib 5.5). It reproduces the original's rules  SRS rotation and wall
kicks, the 7-bag randomizer, lock delay, T-spin detection, combo,
back-to-back, all five modes, the Options menu and the full sound set  while
fixing three rules-layer bugs found in the Python original.

> The original source used for reference lives at
> `_ref\TetraFusion-main\TetraFusion_2.1.py` in the workspace root. The game
> is a single 3,661-line file; this port splits it into a crate-free rules
> layout (see [Project layout](#project-layout)) unit-tested independently of
> rendering.

## Building and running

Toolchain requirements (a stock **Rust stable (MSVC)** install plus these):

| Tool | Why | Where |
| ---- | --- | ----- |
| MSVC Build Tools | Rust MSVC linker (`link.exe`) | `C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools` |
| CMake ≥ 3.x | builds raylib-sys | `C:\Program Files\CMake\bin` |
| LLVM / libclang | clang-sys bindings used at build time | `C:\Program Files\LLVM\bin` |

Builds only need `libclang` on the lookup path:

```powershell
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
cargo run           # play immediately
cargo test          # 394 tests covering the rules layer + live play-throughs
```

The output binary is `target\debug\tetrafusion.exe` (or
`target\release\tetrafusion.exe` after `cargo build --release`). This is a
native windowed app; run it from a desktop session. The exe finds `assets\`
no matter where it is launched from: `cargo run`, a double-click inside
`target\release`, or a copy of the exe anywhere under the project  asset
lookup walks up the directory tree from the working directory *and* the exe
folder. Sound is non-fatal: no audio device, or missing assets, degrade to
silence instead of a crash.

`TF_SMOKE=1` runs the game headless-ish for testing: it auto-hard-drops
twelve pieces, saves `smoke.png` and dumps the exact board to
`smoke_grid.txt`, then exits.

## Controls

| Action | Keys |
| ------ | ---- |
| Move left / right | Arrow keys or `A` / `D` |
| Soft drop | `S` or Down arrow (hold; 20 rows/sec like the original) |
| Rotate clockwise | Up arrow or `W` |
| Rotate counter-clockwise | `Z` or Left Ctrl |
| Hard drop | Space |
| Hold | `C` or Left Shift |
| Menu / confirm | Enter or Space |
| Pause | `P` or Escape (in game) |
| Quit to desktop | Escape (on the menu) |
| Restart | `R` |
| Skip music track | `X` |
| Delete bottom row (Training only) | `H` |
| Toggle fullscreen | `F11` |

The move, rotate, hard drop, hold, pause and skip-track keys are all
rebindable under **Options > Keyboard Keybinds**; the defaults above are just
the starting point. Escape stays bound to pause/quit regardless.

Pausing opens the original's pause menu with **Resume / Restart / Quit to
Menu**: Up/Down (or the gamepad D-pad) to choose, Enter (or **A**) to select,
and `P`/Escape/**Y** resumes instantly. Quit to Menu returns to the main menu;
a new game starts fresh from there.

Gamepad (controller, like the original): plug in a stick and the D-pad and
left stick steer, held down soft-drops, and the face buttons act like the
keyboard  **A** rotates, **B** hard-drops, **X** holds, **Y** pauses. The
D-pad also navigates the menus (**A**/**B** select/back). Button presses move
the piece the same way key presses do; holding either direction uses the same
DAS/ARR as the keyboard. All of it is remappable  see
[Controllers](#controllers).

The mouse is inert during play  controls are keyboard and controller only.
The cursor never steers or clicks the piece, so it can't fight the keys or pad.

DAS/ARR (delayed auto-shift / auto-repeat) default to 150 ms / 50 ms and are
tunable in the Options menu. Up/Down in the menus and on the Options screen
move one entry per press  holding the key does not zip through the list.

## Window sizing and fullscreen

The game is laid out once at a fixed 819×930 virtual resolution and drawn into
an off-screen texture every frame, which is then scaled to fit whatever the
real framebuffer is. Resizing the window, maximising it, or pressing `F11`
therefore just re-fits the same image: it scales to fill, stays centred, keeps
its aspect ratio, and gets black bars on whichever axis has room to spare.

Drawing straight into the framebuffer instead  the obvious approach  only
looks right at exactly one window size. Maximise it and every coordinate stays
pinned to the top-left at its original pixel size, with black filling the rest.

Because the aspect ratio is preserved rather than stretched, a 16:9 fullscreen
display letterboxes the tall playfield left and right. That is deliberate: the
alternative is a stretched, distorted well.

The fit math lives in `render::fit` and is pure, so it is tested directly:
aspect ratio preserved across ten window sizes, result always inside the
framebuffer, smaller windows shrink rather than crop, and a degenerate
zero-sized framebuffer (possible mid-drag) yields a zero rectangle rather than
a NaN one. `TF_WINSIZE=1920x1080` forces a window size so the whole path can
be exercised headlessly.

## Window icon

`assets/ICON1.ico` is **compiled into the executable** by `build.rs`, and that is
the only place the icon can live. A Windows icon is not a runtime property:
Explorer, the taskbar button, Alt-Tab and the title bar all read the icon group
resource in the `.exe`'s resource section, and no runtime call can put one there.

`build.rs` writes a two-line `.rc` and runs a resource compiler (`llvm-rc`,
`rc.exe` or `windres`  searched on `PATH`, then in the usual LLVM and MSYS2
install directories) to produce a linker input that is attached with
`cargo:rustc-link-arg-bins`. The icon is declared twice on purpose:

- `GLFW_ICON` is the exact string raylib's GLFW backend looks for when it
  registers the window class (`LoadImageW(hModule, L"GLFW_ICON", ...)`), which is
  what makes the title bar and taskbar button show it with no runtime code.
- `1` is the conventional numeric group icon that the shell's icon handler looks
  for, which is what makes Explorer show it on the file itself.

If no resource compiler is installed the build still succeeds and the game still
runs; it just carries the default icon. `TETRAFUSION_RC_TRACE=1` reports which
compiler was used and where the compiled resource landed.

### Why the earlier runtime approach was removed

The first version unwrapped the `.ico` container in Rust and handed the pixels to
`SetWindowIcon` at startup. It compiled, reported success, and produced *no icon
anywhere*  in the window, on the taskbar button, or in Explorer. Three separate
things had to be true and none of them were:

- **`SetWindowIcon` cannot reach Explorer or the taskbar entry.** It only replaces
  the running window's HICON. The file in Explorer and a pinned taskbar button are
  read from the resource section, which nothing was writing to.
- **GLFW was overwriting the window icon with a blank one.** GLFW's Win32
  `createIcon` builds a 1-bit AND mask with `CreateBitmap(...)` and never writes to
  it, so the mask is all zeros  which Windows reads as *fully transparent*. The
  resulting HICON is valid, `GetIconInfo` reports `fIcon=True`, and
  `DrawIconEx` renders nothing at all. The runtime call therefore replaced a
  perfectly good default with an invisible icon.
- **The `.ico` is a container, not an image.** raylib's `LoadImageFormat`
  dispatches on the file extension and has no `.ico` case, so `LoadTexture` reads
  every byte correctly and then reports "failed to load the texture". This is why
  the container needed unwrapping in the first place, and why handing the file to
  a resource compiler is the right shape of fix.

Verified against the built binary rather than assumed: `llvm-readobj
--coff-resources` shows a `.rsrc` section containing `Name: GLFW_ICON` and
`Name: (ID 1)` with the 10667-byte PNG payload, and `ExtractIconEx`  the exact
API Explorer consults  returns 2 valid icons instead of 0.

## Modes

From the menu, select one of the original's five, plus **Options** and **Quit**.
Each entry states its own rule underneath, so the mode list is a list of rules
rather than five bare words:

- **Marathon**  15×31 well, normal rules, endless. Level up every 10 lines.
- **Sprint**  clear 40 lines as fast as you can. A running clock is shown; the
  finish time stops when the 40th line lands.
- **Ultra**  three minutes, score as much as you can. A *countdown* is shown
  and turns red for the last half-minute. The run ends **TIME UP**, not game over:
  surviving the full three minutes is not a loss and not a win either.
- **Training**  never ends; a full board resets instead of game over;
  `H` deletes the bottom row. Keeps no high score, because there is no moment at
  which one could be offered.
- **Master**  endless, opens at level 15 with the fastest gravity the game
  allows, and keeps levelling from there.

Master is reachable two ways  the mode, and `Difficulty: Master` in Options 
and both deliver all of it. That was not true: the difficulty option used to give
200 ms/row gravity at *level 1*, so it was fast but scored and coloured like an
opening. See `both_routes_to_master_open_at_level_fifteen`.

The results screen names the ending rather than reducing it to win or lose:
**SPRINT COMPLETE**, **TIME UP**, **YOU WIN** or **GAME OVER**.

| Mode | Ends when | Keeps a high score | Shows a clock |
| ---- | --------- | ------------------ | ------------- |
| Marathon | blocks reach the top | yes | no  it is endless, there is nothing to beat |
| Sprint | 40 lines are cleared | yes | elapsed, frozen at the finish |
| Ultra | 3 minutes expire | yes | time remaining |
| Training | never | no | elapsed |
| Master | blocks reach the top | yes | elapsed |

Every claim in that table has a test that fails if the code stops matching it:
`sprint_ends_exactly_at_its_line_target`, `ultra_runs_for_the_three_minutes_its_blurb_promises`,
`the_clock_stops_when_the_run_ends`, `a_master_run_keeps_levelling_from_where_it_started`,
`each_blurb_agrees_with_the_number_the_game_uses` and
`every_mode_blurb_is_its_own_and_not_empty`.

## High scores

One best score per mode with three-letter initials, stored in `high_score.json`
beside `settings.json` in the file the original used:

```json
{
  "marathon": { "score": 12400, "name": "ACE" },
  "sprint":   { "score": 0,     "name": "---" }
}
```

When a run beats the stored number it goes to the **NEW HIGH SCORE** screen
instead of the game-over screen. Type up to three letters or digits, `BACKSPACE`
to delete, `ENTER` to save and play again, `ESC` to abandon. The original's
**M** for Menu is `ESC` here, since `M` is now a letter.

| Rule | Why |
| ---- | --- |
| Only a strictly better score is a record | A tie is not a record. Ties are common ” same seed, same pieces, same score ” and rewarding one lets a player farm names by replaying a good run until it sticks. |
| A record only ever goes up | Losing to the board cannot delete it, and a worse replay cannot overwrite it. |
| An empty entry does not save | The original guarded this with `if event.key == K_RETURN and initials`, and it is what stops a record being filed as `---`. |
| Non-alphanumerics are dropped | Raylib's default font stops at codepoint 255 and labels are drawn with it directly, so a stored `é` would render as a missing glyph on the pause overlay. |
| Training keeps no record | It never ends, so there is no moment to offer initials at. |
| Missing or corrupt file is an empty table | The table is an enhancement, not a precondition. The game must start. |

The standing record is drawn on the pause overlay as `High: 12400 (ACE)`, as in
the original ” without it a record is invisible except at the moment it was set,
so a player can never tell what they are chasing.

## Options screen

The menu's **Options** entry opens the full settings screen from the
original, cycled with Up/Down and Enter (Escape saves and returns). Settings
persist to `settings.json` in the working directory  the one you launched the
game from, which for `cargo run` is the crate root. Every field has a safe
default, so a missing or corrupt file just resets that option.

| Setting | Choices | Effect |
| ------- | ------- | ------ |
| Keyboard Keybinds | submenu, 8 actions | Remaps every rebindable action |
| Controller Keybinds | submenu, 8 actions + Menu Nav | Remaps every controller input, on the stick and on the D-pad |
| Difficulty | easy / normal / hard / very hard / master | Base gravity |
| Theme | 7 palettes (verbatim from the original) | Board + block colours |
| Piece Colours | Adaptive / Traditional | Whether piece colours morph with the stage |
| Piece Skin | Follows Stage, or pin one of 5 | The material blocks are drawn in |
| Effects | flame / wind / water / ice / flicker / matrix / none | Particles trailing the piece, and the gesture bursts |
| Gravity | 0.5x - 3x | Multiplier folded into the fall speed |
| Ghost piece | on/off | Drop-shadow of the current piece |
| Ghost opacity | 0-255 (default 80) | Ghost alpha |
| DAS / ARR | 50-400 ms / 30-200 ms | Held-key steering speed |
| Screen shake | on/off | Shake on line clears (8 px + 3 px/line, as in the original) |
| Backgrounds | on/off | The per-level background photos. Off draws the plain backdrop |
| Grid lines | on/off | The playfield lattice, all or nothing |
| Grid opacity | 255 / 0 / 64 / 128 / 192, cycling | Lattice alpha: visible, barely, none |
| Music | on/off | Background track toggle, live-applied |
| Use Custom Music | on/off | Play a folder of your own tracks instead of `Background.ogg` |
| Music Folder | folder picker | Which folder the custom tracks come from |

The row pitch is derived from the row count (`render::settings_layout`) rather
than hardcoded, so adding an option later cannot push the last row off the
bottom of the screen.

Row text is *measured*, not eyeballed. `center` used to place a label at
`(SCREEN_WIDTH - width) / 2`, which is negative for anything wider than the
450px column  and `draw_text` accepts a negative x without complaint, so an
over-long label did not look tight, it silently lost its first few characters
off the left edge. That is how `Controller Keybinds: 8 of 8 bound - auto` came
out as `oller Keybinds: 8 of 8 bound - auto`. Now the width is measured and the
font is shrunk to fit, once for the whole column so every row stays the same
height as its neighbours. `render::fit_size` is pure (it takes a width function
rather than measuring, so it is testable without a window) and
`render::centered_x` is the clamp that pins the off-the-edge case.

`cargo test` also checks that no row has outgrown the column: if one has, the
list renders smaller rather than clipping, which is correct but silent, so a
test measures the longest default row against the real font's advance (12.4px
per character at 24pt, taken from `MeasureText`, not estimated).

### Effects

**Options > Effects** picks the trail that follows the piece while it is
steered or soft-dropped. Nothing trails during passive gravity, and steering
into a wall still smokes  both are the original's, because the original spawned
from *held input state* rather than from whether a move landed. Dust on a hard
drop and debris on a clear are separate systems that run whatever the Effect is
set to. The same six Effects also shape the [gesture
bursts](#gesture-particles) thrown by a slide, a rotation or a slam.

`matrix` is the one effect deliberately improved on. The original drew a
1-pixel-wide green bar, 615 pixels long, interpolating from bright head to
faint tail  so the option named Matrix produced a green smear rather than
anything recognisable. This port draws **halfwidth katakana** falling in a
column instead, re-rolling every 4 frames so the rain scrolls, bright at the
leading edge and fading up the tail. The gradient is also the other way round
from the original's: the original put the bright head at the top of a falling
streak, which reads as code being dragged upwards.

The symbols are a hand-drawn 5×7 bitmap table (`KATAKANA` in `src/effects.rs`),
not text, and the reason is a hard font limit: raylib's built-in font has glyphs
for codepoints 32255 only (`defaultFont.glyphs[i].value = 32 + i` in
`rtext.c`), and halfwidth katakana is U+FF66U+FF9D. The only font asset the
game ships is a tetromino font with no kana either. Drawing the cells as
rectangles means no font dependency, no atlas, no runtime codepoint mapping, and
the same result on every platform.

Two tests hold the table honest, because there is no way to review 34 glyphs by
reading hex: one rejects a blank glyph (which would just silently never appear
in the rain) and one rejects a row that sets a bit outside its five columns
(which would be drawn on the neighbouring glyph).

### Rebinding keys

**Options > Keyboard Keybinds** lists all eight actions with their current key.
Enter starts a capture - the row turns into "...press a key..." and the next key
you press becomes the binding. Escape cancels instead of binding, so rebinding
can never take away your way out of a run.

| Action | Default |
| ------ | ------- |
| Move Left | Left arrow |
| Move Right | Right arrow |
| Soft Drop | Down arrow |
| Rotate | Up arrow |
| Hard Drop | Space |
| Hold Piece | `C` |
| Pause | `P` |
| Skip Track | `X` |

Notes:

- Escape stays hardwired to pause/quit even if you rebind Pause, so a bad
  rebind cannot lock you into a run.
- The WASD / `Z` / `C` / Shift alternates listed under Controls keep working
  alongside whatever you bind.
- Bindings are stored as raylib keycodes. raylib's `KeyboardKey` is a bindgen C
  enum with no inverse conversion and non-contiguous discriminants, so
  `src/keys.rs` holds an explicit table that a test checks against the real
  enum. A `settings.json` hand-edited to a code with no matching key falls back
  to that action's default instead of reaching the input layer.

### Controllers

Every controller input is remappable, on the buttons and on the sticks
alike  nothing is hardwired. **Options > Controller Keybinds** has the eight
in-game actions plus a **Menu Nav Bindings** submenu for the four menu buttons,
mirroring the original's three settings blocks (`controller_controls`,
`controller_menu_navigation`, `controller_settings`).

Enter on a row starts a capture; the next button *or stick direction* becomes
the binding. **Back** cancels, so a rebind can never take away the way out of a
menu  and Back is itself rebindable, which is why its own row is a capture row
like the rest.

| Action | Default | Also available |
| ------ | ------- | -------------- |
| Move Left | D-pad left | Left stick X, right stick X, right trigger as a digital 17 |
| Move Right | D-pad right | Left stick X, right stick X, left trigger as a digital 16 |
| Soft Drop | D-pad down | Left stick Y, right stick Y |
| Rotate | A | Left stick click, right stick click |
| Hard Drop | B | D-pad up |
| Hold Piece | X | Start / Back |
| Pause | Y |  |
| Skip Track |  | Left shoulder / right shoulder |

The stick and the D-pad are **additive, not alternatives**: whichever you push
moves the piece, and both are live at once. Every press is an edge
(`now && !last`), so a held direction moves the piece once and then hands over to
DAS/ARR instead of repeating at the frame rate. `use_dpad` in the settings
gates only automatic piece steering  the D-pad always navigates menus.

**Controller Settings** holds the stick threshold, the deadzone, whether the
D-pad steers the piece, the repeat delay, and which slot the game reads. Slot 0
means "first available", so a wheel or a flight yoke in the first slot does not
stop a controller in the second one from being used.

Three notes on the original's settings, all of which are bugs the port fixes
or hardens:

- The original's D-pad defaults were `left: 14, right: 15, down: 13`  which
  are D-pad *right*, an unmapped MISC1 button, and D-pad *left*. Left and right
  were swapped and Down steered the wrong way. Corrected to 13/14/12. The menu
  nav defaults (11/12/0/1) were right and are kept.
- Bindings are stored as SDL `SDL_GameControllerButton` numbers, so a
  `settings.json` written by the Python original reads correctly here. Three
  shapes round-trip: a bare number is a button, `["hat", [x, y]]` is a hat, and
  `["axis", n, "positive"|"negative"]` is a stick or trigger direction.
  Anything unrecognised falls back to that action's default rather than
  silently unbinding the action.
- The original's hat support has no equivalent in raylib, which exposes no hat
  API at all  a `["hat", [x, y]]` binding resolves to the D-pad buttons
  (11/12/13/14) here, which is what it meant on every pad the original ran on.
  The `hat_threshold` value is kept so the file still round-trips.
- Thresholds are clamped to `(0.01, 0.99)`, not to 1.0. A real stick at full
  tilt commonly reports 0.98, so a threshold of 1.0 is unreachable and is
  indistinguishable from a dead stick.

### Level tinting

Piece and block colours shift as the level climbs, as in the original  the
board cycles through a full colour rotation roughly every 25 levels rather
than staying one flat palette. `render::level_tint` adds a per-level offset of
`int(12·sin θ)`, `int(8·sin(θ+2.094))` and `int(10·sin(θ+4.189))` to the red,
green and blue channels, where `θ = level × 0.25` radians, clamped to 0..255 per
channel. Threaded through `draw_3d_block` and `draw_piece`, and the hold and
next previews go through the same path  they used to read the piece's raw
colour index, so the previews disagreed with the piece from level 2 onwards.

## Piece colours

There are two independent colour settings, and the difference between them is
the point:

- **Theme** is the original's 7 palettes, verbatim. It is an accessibility
  feature  high contrast, monochrome, and so on  and it was not replaced.
- **Piece Colours** is a two-way choice between palettes that morph with the
  level and standard Tetris colours.

`palette_for_settings(theme, mode, level)` is the single entry point, so there
is no way for one draw path to read a theme while another reads a palette mode.

**Adaptive** (the default) starts from the theme's palette and rotates hue by
`STAGE_HUE_STEP = 0.12` turns (≈43°) per level, with saturation and value
riding two further sines so the result does not read as a mechanical rotation.
Both sines, not one: a cosine is at its maximum at level 1, which silently
brightened the very first level a player ever sees by 12%. Piece colours also
rotate which palette slot they read, which is the original's behaviour and is
kept.

**Traditional** hands back a fixed standard-Tetris palette  I cyan, J blue,
L orange, O yellow, S green, T purple, Z red  and ignores both the theme and
the level. This is the escape hatch for anyone who wants the new look without
losing the one thing the original was good at: knowing what a piece is from a
glance. Backgrounds, skins and particles all stay exactly as they are; only the
seven piece colours are pinned.

Pinning the palette also pins the *slot*, not just the colours. `color_for`
otherwise adds `level - 1` to the palette index, so a "Traditional" I piece
would still have changed colour every ten lines  the option would have
promised something it did not deliver. Turning the shift off is therefore part
of choosing Traditional, and a test walks the whole run to prove no piece ever
changes. *Regression tests:* `a_pinned_palette_gives_every_piece_the_same_colour_all_run`,
`a_pinned_palette_still_separates_the_seven_pieces`,
`the_default_palette_shifts_with_the_level`.

## Piece skins

**Options > Piece Skin** chooses the *material* a block is drawn in, as opposed
to its colour. Twelve skins, in `src/skins.rs`, listed most-solid to most-flat �
which is also the order a run is played through:

| Skin | Block looks like | Relief |
| ---- | ---------------- | ------ |
| Classic | The original's gloss block, unchanged | 1.00 |
| Chrome | Mirror metal: a hard horizon, bright sky over dark ground | 0.95 |
| Ember | Charred body with molten cracks, extruded like a lump of coal | 0.85 |
| Crystal | Faceted, translucent, glows toward white on a pulse | 0.70 |
| Prism | Spectral: four wedges of hue rotating as it pulses | 0.60 |
| Venom | Solid body with glowing drips running down from the top | 0.50 |
| Abyss | Underwater - banded depths, a slow caustic wave | 0.45 |
| Slate | Matte and quiet, almost no contrast, no gloss at all | 0.30 |
| Marble | Pale stone with white veins that breathe along their seams | 0.20 |
| Wire | Terminal phosphor: dark body under a diagonal hatch | 0.10 |
| Halo | Hollow: one breathing ring inside an outline | 0.00 |
| Neon | Minimalist glowing line-art, mostly empty | 0.00 |

**Follows Stage** (the default) gives **every level its own skin** �
`LEVELS_PER_SKIN = 1`, so the cycle runs twelve levels before wrapping. It was
`4`, then `2`, both of which were long enough that a player could climb several
levels without the board looking any different: the level-up arrives, the banner
lands, and the pieces look exactly as they have for the last twenty lines.

### Flat and solid: the 2D/3D mix is declared, not accidental

`Skin::relief()` is the single source of truth for how extruded each skin's
blocks are, and `draw_block` passes it straight through to a shared `bevel()`
helper. This is a correction. In the original five, only `Classic` ever had the
extruded right-and-bottom faces written down; `Ember` and `Abyss` drew a flat
square with some lighting on top, and `Neon` and `Crystal` were flat for a
different reason. So "a different block type per stage" meant four stages that
were the same square in different colours.

Ten of the twelve are now genuinely solid, at ten different depths, and the two
outline-only skins sit deliberately at `0.0` � they are *drawn shapes* rather than
lit solids, and a bevel on them would be a lie about what they are. `the_skin_list_is_a_descending_ladder_of_dimensionality`
holds the order in place, so the run always walks down the ladder instead of
stuttering.

Two things the tests caught that were not visible from reading the code, and are
worth knowing about if these skins are ever extended again:

- **A skin must not merge two piece colours.** `Chrome` originally pulled both
  faces 58% toward white and `Marble` lifted the body 46% toward white; both
  collapsed cyan and blue into the same pastel. In a stacking game colour is how
  the board is read at a glance, so that is not a look, it is a difficulty spike.
  `Chrome` now gets its depth from the top/bottom ratio and `Marble` puts its
  paleness in the veins instead of the body.
- **A glow that is computed but never drawn still passes the palette tests.**
  `Marble` had a breathing `glow` ramp that no draw call referenced. The tests
  only read `skin_palette`, so they were satisfied by a colour nobody saw.
  `two_pieces_stay_apart_in_every_skin` and `no_two_skins_share_a_glow_ramp`
  guard the first kind of mistake; the second is why each draw function's use of
  `p.glow` is commented at the call site.

*Regression tests:*
`the_skins_mix_flat_and_solid_blocks`,
`the_skin_list_is_a_descending_ladder_of_dimensionality`,
`the_solid_skins_do_not_all_share_one_extrusion`,
`no_two_skins_share_a_glow_ramp`,
`every_skin_is_reachable_inside_a_short_run`,
`neighbouring_levels_never_look_the_same`.

Pin a skin and it stays pinned for the whole run, at every level; "Piece Skin:
Neon" at level 60 is still Neon. *Regression tests:*
`a_pinned_skin_survives_the_level_climbing`,
`the_automatic_skin_follows_the_stage`,
`the_piece_skin_row_offers_automatic_and_every_skin`,
`every_skin_label_resolves_to_the_skin_it_names`.

Two structural decisions make this testable without a window:

- **Skins are pure colour recipes.** `SkinPalette` is `[u8; 3]` values built by
  `scale`, `blend` and `luma` and converted to a `Color` at draw time, so a
  skin's entire appearance is a handful of numbers that a test can assert on.
- **`Look` bundles skin and pulse together.** `Look::new(skin, frame)` is the
  only constructor, which makes it structurally impossible for a draw call to
  combine one stage's material with another stage's pulse.

`draw_block` decides geometry only; every colour it uses comes from
`skin_palette`. That separation is why the skins could be added without
touching `render.rs`'s shading, and why the dead `shade`/`mix` helpers there
were deleted instead of left as a second source of truth.

Two rules are enforced rather than assumed, both of which were wrong in a first
draft and caught by tests:

- **Glow mixes toward white as the pulse rises**, never away from it. Mixing
  away made green's "sparkle" get *darker* as the block brightened.
- **Classic never pulses.** It is a faithful reproduction of the original's
  look, and a faithful reproduction does not gain a heartbeat.

Distinguishability between skins is tested on **hue**, not raw channel
distance. Dark-body skins compress toward black, so an absolute brightness
threshold fails on Neon purely because `base * 0.14` rounds channels down.

## Gesture particles

The Effects trail fires from *held input state*, which means a single tap and a
rotation produced nothing at all. **Gestures** are the second particle system,
and they fire on the event instead  a burst whenever the piece slides, rotates
or is slammed, anchored to the edge the piece travelled toward, because that is
the edge that swept into new space. From the centre the particles would spend
most of their life hidden inside the block.

Intensity scales with the level, as the stage does:

```
gesture_power(level) = max(level / 20, 0.15)
gesture_count(level) = round(2 + 10 × power)     // 4 at level 1, 12 at level 20
```

Both ends of that are load-bearing. The floor stops level 1 throwing nothing 
a gesture that produces no particles reads as the Effect setting being broken,
not as "subtle". The ceiling stops a late run putting hundreds of particles on
every keypress. *Regression tests:*
`the_burst_scales_with_the_stage_and_stops_at_the_ceiling`,
`a_gesture_always_throws_something`.

The six Effects keep their own silhouettes rather than becoming one shape in
six colours: water spreads as a flattened ring and lingers, ice throws fast
short shards, flame rises whatever the piece was doing, wind and flicker follow
the gesture's direction, matrix falls away and scrolls. A ripple is *drawn* as
a ring with `draw_circle_lines`; faking one by overdrawing a black disc would
paint a dark blob over the board.

Every shot is a pure function of `(kind, direction, level, t)`. `t` walks a full
turn, so a burst spreads in all directions and never piles up, and no random
number is involved  which is what lets the shapes be asserted on at all.
*Regression tests:* `a_gesture_is_reproducible`,
`the_effects_move_in_visibly_different_directions`,
`a_ripple_spreads_wider_than_it_spreads_tall`,
`a_burst_is_thrown_from_the_piece_it_came_from`,
`no_effect_means_no_gestures`.

Soft drop deliberately does *not* throw a gesture. It is gravity, gravity fires
on every row including passive falling, and treating it as a gesture would
spray a burst once per row of a piece doing nothing. The held-key trail covers a
soft drop and a hard drop  the drop a player actually chose  gets its slam.

## Level-up transition

The original's level change re-randomises the stack's colour indices every
100 ms for two seconds. That is a hue storm, and it is retained  but on its own
it is not a transition, because the *shape* of the board is identical before and
after. There is nothing to look at except the colour of blocks that have not
moved, and once the player has seen it twice it stops registering as an event at
all. Two things were added on top.

### The stack loses its colour, and the edges keep it

Falling pieces drain: colour leaves the interior of each block and the edges stay
lit, so the board reads as a grid of glowing outlines rather than going flat
grey. The settled blocks, the live piece, its ghost and the hold/next previews
all drain together on the same clock.

That last part was the bug. The drain was originally applied only to blocks
already at rest, with the live piece, its ghost and the previews pinned to
`(1.0, 1.0)` at the draw call � on the reasoning that the block in flight has to
stay maximally legible at 20G. The effect was that the stack went grey while the
piece the player was steering stayed fully coloured, so the whole thing read as a
background animation instead of an event happening to the game. Pieces now go
through `piece_keep(fx, t)`, which is `fx_keep` at `exposure = 0.0` and
`height = 1.0`: a piece in open air has no neighbours and sits above the stack,
which is its honest geometry, and makes it behave like any other block. The
legibility concern is answered by the rim floor rather than by an exemption �
`fx_keep` never lets a rim drop below `MIN_RIM_KEEP`, so the falling piece keeps a
lit outline on every frame of the transition.

```
drain_phase(t)  0 → 1 over the first 55% of the window, then 1 → 0 over the rest
keep = 1 - drain_phase(t)
```

The triangle is a triangle on purpose. The recovery leg is what gives the level
change an *ending*  a drain that only ever goes one way leaves the board grey
and the player waiting for something to happen, and one that snaps back instantly
is a blink. `LEVEL_TRANSITION_MS` is unchanged at 2000, and the drain, the hue
storm and the banner all ride one clock rather than three that can drift apart.

Three variants, one per level, chosen by `stage_fx(level)`:

| Variant | What drains | Reads as |
|---|---|---|
| `Rim` | The interior, uniformly. The rim stays at full colour the whole time. | A lattice of lit edges. |
| `Depth` | Buried blocks first (`Grid::exposure`); surface blocks last. | The stack hollowing out from the inside. |
| `Wave` | A band of width 0.25 that starts on the floor and climbs the stack. | A wavefront crossing the board. |

`stage_fx` steps by 2 mod 3, so the order is `Rim`, `Wave`, `Depth` and no two
adjacent levels pick the same one. That is **not** `level % 3`: the skin cadence
(`LEVELS_PER_SKIN`, also 4) and the palette hue step would then change in lockstep
with the effect, and all three would read as a single event rather than three.
*Regression tests:* `every_level_names_an_effect_and_neighbours_differ`,
`a_level_outside_the_scale_still_names_a_real_effect`.

The three are genuinely different functions, not one effect in three clothes 
`Depth` is blind to height, `Wave` is blind to exposure, and `Rim` is flat across
the whole board. *Regression tests:*
`the_three_effects_are_measurably_different`,
`depth_holds_the_surface_longest`,
`the_wave_climbs_the_stack`.

The sign on `height` in `Wave` is load-bearing and is what `the_wave_climbs_the_stack`
exists to protect: with `(1.0 - height)` the band is identical in a still and
runs the wrong way in motion, and the eye expects a wave to fall.

### Nothing ever becomes invisible

The rim is floored at `MIN_RIM_KEEP` (0.55), and the floor lives in `fx_keep` rather
than in the renderer so the promise the function makes is the one the player
sees. A block whose rim reached zero would be indistinguishable from an empty
cell, and the player's next decision depends on telling those two apart at a
glance. Drained colour is a *darkened grey at the same luminance*, not a scaled
hue  scaling the channels reads as "the block is in shadow", which looks like a
lighting bug, where losing saturation reads as what it is.
*Regression tests:* `a_fully_drained_block_still_has_a_lit_edge`,
`the_rim_outlasts_the_body`, `a_fully_drained_block_is_grey_rather_than_black`,
`a_fully_kept_block_is_exactly_its_colour`.

**The live piece, its ghost and the hold/next previews never drain.** A level-up
happens exactly when a new piece arrives, and at 20G the player has a fraction of
a second to read it  so the one block that has to stay maximally legible is the
one in flight. The drain and the tint both live in `draw_3d_block` rather than in
`skins::draw_block`, so all five materials get the treatment without any of them
knowing a level-up exists.

`Grid::exposure` counts eight neighbours, not four. Orthogonals alone put a block
lying on a flat surface at 0.5  "half-buried"  for a block that is entirely in
view, and the diagonals are what separate *resting on* the stack from *wedged
into* it. Off-board counts as open sky, not as a wall.
*Regression tests:* `exposure_spans_its_full_range`,
`a_block_on_a_flat_surface_reads_as_exposed`,
`the_edge_of_the_board_reads_as_open`, `exposure_ignores_the_cell_its_own_contents`,
`exposure_is_always_a_real_fraction`, `an_empty_board_has_no_span`,
`the_span_covers_exactly_what_is_stacked`,
`the_span_accounts_for_the_hidden_rows`.

### The banner

`draw_level_banner` is the part that says *you reached the next level*. The text
punches in from 30 px to 74 px on a cubic ease-out over the first 16% of its
life, so the arrival has a physical edge instead of just appearing. A bright
band sweeps the full height of the board, giving the transition a direction to
travel in, and the whole thing is tinted with the stage's own palette entry, so
the banner is visibly *of* the level the player just reached rather than a generic
overlay. Under the level number it prints the gravity the level now runs at, so
"Level 7" and "what Level 7 means" are both on screen at once.

### Game over accepts P, and says so

`P` puts the player back into the game. It used to mean "back to the main menu"
here, which made the pause key the one key on this screen that could not resume: a
player who finished a run and reached for `P`, as they do at every other pause in
the game, was silently doing nothing unless they happened to know `Enter` was the
way in. The screen now names every key that does something:

```
ENTER or SPACE - PLAY AGAIN
P              - PLAY AGAIN
ESC            - MAIN MENU
R              - RESTART IN THIS MODE
```

`Escape` reports as both `quit` and `pause`, because it is the pause key
everywhere else in the game, so the routing is a pure `over_action` that tests
`quit` *first*. Testing `pause` first would turn `Escape` into "play again", and
a player who had bound Pause to the same button as Back  which the pad bindings
allow  would have no key that leaves the screen at all. The win screen is the
same screen with a different title and routes identically.
*Regression tests:* `p_resumes_on_the_game_over_screen`,
`escape_leaves_rather_than_resuming`,
`every_advertised_key_does_what_the_hint_says`, `an_idle_game_over_screen_does_nothing`,
`a_win_screen_routes_exactly_like_a_loss`.

### Events carry their own pose
`Event::Move`, `Rotate`, `HardDrop` and `Lock` all carry a `Pose`  the piece,
its rotation and its origin at the instant the event happened. That is not
tidiness. Events are read a frame or more after they are pushed, and a hard
drop locks the piece *and* spawns its successor in the same call, so the live
`origin` is a brand new piece at the top of the board. Reading live game state
to place an effect puts the hard-drop burst at the ceiling instead of the
floor  and does it only at high gravity, which is exactly where the effect
matters most. *Regression tests:*
`every_pose_event_reports_the_piece_as_it_was`,
`a_hard_drop_event_reports_the_piece_that_was_dropped`,
`a_pose_is_always_a_position_the_piece_actually_held`.

### Custom music

Two settings, as in the original. **Use Custom Music** switches the bundled
`Background.ogg` loop out for a folder of your own files; **Music Folder**
picks which folder. Both apply immediately, so you can hear the change without
starting a run.

**Music Folder** opens the operating system's own folder picker, so pressing
Enter hands the choice to File Explorer and every drive on the machine is
reachable:

| Platform | Dialog |
| -------- | ------ |
| Windows | `IFileOpenDialog` with `FOS_PICKFOLDERS` (the Explorer folder browser) |
| macOS | `NSOpenPanel` in folder-picking mode, via AppleScript, as the original did |
| Linux | `zenity --file-selection --directory`, as the original did |

The dialog opens in whatever folder is already set, or your home directory the
first time. Cancel leaves you where you were with the old folder intact.

An earlier draft replaced this with a keyboard-driven browser built into the
game. That was a mistake and it is gone: the browser could only ever see the
tree under one drive letter, and it was strictly worse to use than the dialog
the operating system already provides.

`src/folderpick.rs` holds the platform layer. On Windows it talks to COM
directly rather than through a crate, because every Windows dialog crate
reaches the API through `windows-sys`, whose generated `user32` bindings also
declare `CloseWindow`  which raylib's own C code defines, so the release build
fails to link with LNK2005. Declaring the handful of entry points by hand keeps
the link against `ole32` and `shell32` only. The decision about what a pick
*means* is a separate pure function, so the "cancel is not the same answer as
this machine has no dialog" rule is pinned by a test rather than by reading the
call site.

Once a folder is set, tracks play one after another rather than looping, and
the **Skip Track** key (default `X`, rebindable) jumps to the next one. The
Music row shows what is playing and where it sits in the list. Music stops on
the game-over screen, exactly as the bundled track did.

Playing each track once is not raylib's default and takes two lines of code to
get right. `LoadMusicStream` sets `Music.looping = true`, and
`UpdateMusicStream` only calls `StopMusicStream` when `looping` is false  with
it true the stream wraps internally (`framesProcessed % frameCount`) and keeps
reporting "playing". So a custom track never *ends*, the playlist's
"did the last track finish?" check is never true, and the second track is
never reached. `audio::set_looping(false)` on custom tracks is the fix; the
bundled `Background.ogg` is meant to loop and keeps the default. Mute and
unmute restart a track rather than consuming one, which is why the decision is
`Start` and not `Advance` when a track is disarmed.

The scanning rules are the original's, verbatim:

- Extensions `.wma .wav .ogg .mp3 .aac .flac .m4a`, matched case-insensitively.
- Top level only - subfolders are not searched, though the browser can walk
  into one.
- Hidden files (a leading dot) are skipped.
- Sorted by the original's key: names starting with a digit first, then names
  starting with a letter, then everything else, ties broken case-insensitively.
  (So `"10.mp3"` sorts before `"9.mp3"`. That is what the original did.)
- Capped at 500 files so a huge folder cannot stall the game.

Anything that goes wrong falls back to the bundled track rather than to
silence: an unset folder, a folder that has been moved or deleted, a folder
with no playable audio, or a file raylib cannot decode (the playlist walks
past up to eight bad files before giving up). `src/music_dir.rs` holds the
scanning and playlist logic; the parts that can be checked without an audio
device are tested directly, including against real folders on disk.

### Grid visibility

The playfield lattice is controlled by two settings, matching the original's
`grid_lines` and `grid_opacity`. **Grid Lines** switches it off entirely;
**Grid Opacity** steps 255 -> 0 -> 64 -> 128 -> 192 -> 255, which is the
visible / barely visible / not visible range. Opacity 0 and Grid Lines off both
mean "no lattice", so the two settings agree.

## Audio

All of the original's sound assets are used with their original triggers:

- `Background.ogg` loops through the menus and while a run is live, unless
  **Use Custom Music** is on  see "Custom music" under Options for the
  folder, the track order and the Skip Track key.
- `Lineclear.ogg` / `MultipleLineclear.ogg` play for 1-3 line clears and
  Tetrises respectively.
- `GAMEOVER.ogg` plays once when a run ends.
- `heartbeat_grid_almost_full.ogg` pulses while the top four rows are
  occupied (the "danger zone"), and stops the moment it clears.

The `assets/` folder next to the executable ships with the original's sound
files (plus its `tetris-blocks.TTF`). Missing files degrade to silence, never
a crash. Every 10 lines the run levels up: gravity climbs to 20G by level 20
(see [Gravity](#gravity)), the piece skin and palette move on, and the whole
stack flashes through random colours for two seconds.

One raylib detail is load-bearing for the bundled loop. `LoadMusicStream` sets
`Music.looping = true` and `UpdateMusicStream` only stops and rewinds when
`looping` is false  when it is true it wraps internally with
`framesProcessed % frameCount` and the stream keeps reporting "playing". A
custom playlist has to play each track *once* and move on, so `src/audio.rs`
calls `set_looping(false)` on custom tracks and leaves the bundled track alone.
The decision of what to do next is a pure function, `audio::music_action`,
returning `EnsurePlaying` / `Wait` / `Advance` / `Start`, which is what makes
the advance path testable without an audio device; a named regression test,
`a_custom_playlist_advances_instead_of_repeating_one_track`, pins it.

## Scoring and rules (as in the original)

- Line clears: 100/level per line; T-spin: 400/level; all-clear: 1200/level;
  hard drop: 2/row; combo: `50 × combo × level × lines`.
- Back-to-back: consecutive T-spin or Tetris clears add a half-bonus on the
  running score, exactly as the original did.
- A four-line clear lights **TetraFusion!** in the side panel for 2 s, in a
  colour re-picked every frame, matching the original's `draw_subwindow`
  flash.
- Gravity curve per level: see [Gravity](#gravity) below.
- Soft drop runs at a fixed 50 ms/row (`current_fall_speed = 50 if fast_fall
  else fall_speed`), not one row per frame.
- Level is driven purely by clears: `lines_cleared / 10 + 1`.
  *Regression test:* `every_ten_lines_advances_one_level`.
- SRS kick tables for J/L/S/T/Z and I are transcribed verbatim; O does not
  kick. Rotation follows the SRS 3-corner T-spin rule.
- Every rotation state is the previous one turned 90° about the centre of the
  piece's own fixed box (4×4 for I, 3×3 for J/L/S/T/Z, 2×2 for O). The kick
  offsets are only meaningful if all four states agree on where that centre
  sits. *Regression tests:*
  `every_rotation_is_a_rigid_turn_about_the_box_centre`,
  `every_rotation_is_the_right_tetromino`.

## Gravity

The original's curve was `max(50, base × 0.85^(level−1))`, which bottoms out
on a 50 ms floor and then stops moving. Past roughly level 20 a run was played
at the same speed whether it was level 22 or level 200. This port replaces it
with a ramp that actually arrives somewhere:

```
gravity_g(level)             = clamp(level, 1, 20)            // 1G to 20G
fall_speed_for(base_ms, level) = max(round(base_ms / gravity_g(level)), MIN_FALL_SPEED)
MIN_FALL_SPEED               = 50     // which *is* 20G at a 1000 ms base
MAX_GRAVITY                  = 20
```

Every difficulty reaches exactly 20G at level 20. The floor is what stops a low
base going faster than 20G; the ceiling is what stops a high base going
faster, which is deliberate  20G is the top of the scale the game advertises.

Intervals are **rounded**, not truncated. The original's `int()` is a bug at
this end of the range  it discards a whole millisecond out of every interval 
and the point of a 20 ms cadence is that it is a cadence, so it is rounded to
the nearest real millisecond.

Two details are load-bearing and pinned by tests:

- **A game opens at its raw base speed.** `fall_speed_for(base, 1) == base`, so
  the curve is identical at spawn for every mode that opens at level 1, and a
  player starting a run gets exactly the first drop they have always gotten.
  Master is the deliberate exception: it opens at level 15 and keeps its own
  base speed until its first level-up, matching the original. It then keeps
  levelling *from 15* ” the original's rule is `lines // 10 + 1`, which reads 1
  until 141 lines and so never exceeds a level-15 opening, freezing Master at
  level 15 for the whole run. Here that also froze the skin ladder, since every
  level wears a different skin. The level is therefore offset by
  `Settings::start_level`, which is identical to the original's rule for every
  mode that opens at 1. *Regression tests:*
  `gravity_starts_at_the_base_speed_for_every_difficulty`,
  `master_opens_at_its_base_speed_not_the_level_15_curve`,
  `a_master_run_keeps_levelling_from_where_it_started`,
  `an_ordinary_run_levels_from_one_unchanged`.
- **At 20G a piece does not fall  it lands.** `spawn_piece` puts a new piece
  on the floor in the same frame it appears (`Game::at_max_gravity`). That is
  the intended climax of the ramp: the last level is a placement game, not a
  reflex game, and lock delay and floor-sliding are the only things that
  still matter. It lives in `spawn_piece` rather than in the gravity timer so
  it covers *every* spawn, including the one at the top of a frame where no
  gravity tick happened to be due. *Regression tests:*
  `a_piece_spawned_at_20_gravity_lands_on_the_floor_immediately`,
  `only_20_gravity_spawns_instantly`.

The curve is not a straight line on screen even though it is one in
`fall_speed_for`, because the cadence is quantised to whole milliseconds: 400 ms
a row and 396 ms a row are the same piece to a player. The early levels
therefore read as a handful of distinct steps rather than a slow constant
creep, and that is the level design  `fall_speed_for` is where it lives, and
it is a single expression so the two are the same thing.

## Backgrounds

Each level gets its own background photo, chosen at random the first time the
level is reached and then held for the rest of that level. Consecutive levels
never draw the same image while alternatives remain.

Images live in `assets/backgrounds/N.jpg` and are all loaded once at startup.
The game is fully playable without them: a missing folder or an undecodable
file is skipped, and the plain backdrop is drawn instead. The photo is
cover-fitted (scaled to fill, centred, overflow cropped) so no letterboxing
shows, then dimmed by `DIM` in `src/backgrounds.rs`  that dim is load-bearing,
since blocks, the ghost piece and the grid lines must stay readable on top of
an arbitrary picture. It is 100 of 255, about 39%: heavy enough to flatten any
photo into a backdrop, light enough that you can still see the photo. Raise or
lower it there to taste. **Options > Backgrounds** turns the photos off entirely
for a plain backdrop, without unloading them, so switching back on is instant.

To add your own, drop numbered JPEGs into `assets/backgrounds/`. Numbering
stops at the first gap.

## Bug fixes (deliberate deviations from the original)

The port intentionally does **not** replicate these defects in the 2.1
source:

1. **Non-hard-drop locks scored nothing.** The original had three
   near-identical lock handlers (gravity, soft drop, hard drop) and only the
   hard-drop path awarded points. This port has one `lock_piece()` and every
   lock scores identically. *Regression test:*
   `soft_drop_lock_scores_and_advances_the_level`.

2. **Game over almost never triggered.** The original's `check_game_over`
   tested `grid[0]`  the very top row of a 31-row board, roughly 28 rows
   above where pieces spawn  so a stack almost never reached it. The port
   instead tops out when the stack grows into the hidden spawn buffer, and
   treats a piece that locks entirely above the playfield as a lock-out.
   *Regression tests:* `topping_out_ends_a_marathon`,
   `stack_in_the_buffer_tops_out`.

3. **T-spin detection was wrong for most orientations.** The original only
   recognized the spawn orientation of the T and read cells the T did not
   occupy. The port applies the SRS 3-corner rule for every rotation, checked
   against the board *before* the T is placed so its own cells cannot count
   as blocked corners. *Regression tests:*
   `t_spin_detection_works_in_every_rotation`, `a_plain_t_drop_is_not_a_t_spin`.

Other corrections made while porting:

- **Floor rendering.** The board renderer mapped rows with a 60 px top offset
  that pushed the floor rows off the bottom of the window; visible rows are
  now mapped to `(y − hidden_rows) × block`, so the stack sits on the floor.
- **Soft drop speed.** The original capped fast-fall at a 50 ms/row cadence;
  an early port draft dropped a row every frame (~60 rows/sec), making a held
  Down feel manic. It now falls at the original's twenty rows per second.
- **Menu navigation.** Menu and Options Up/Down move one entry *per press*
  by reading the key-press edge, not the held state  the original's Pygame
  event loop did exactly that.
- Lock delay now starts the moment a piece comes to rest. In the original (and
  an early port draft) a piece sat for nearly a full gravity interval after
  landing before the lock window even began counting.
- The game starts with the actual first piece from the 7-bag, not a hardcoded
  T.
- Master mode's starting speed is derived by the same ramp as every other mode,
  but it is the one mode exempt at spawn: it opens at level 15 and keeps its own
  base speed until its first level-up, which is what makes the ramp apply to
  Master where it does not apply to Marathon. *Regression test:*
  `master_opens_at_its_base_speed_not_the_level_15_curve`.
- **Asset discovery.** `find_asset` walks up the directory tree from the
  working directory and the exe folder, so the game finds `assets\` whether
  it is run from the crate root, `target\release`, or anywhere in between.
- **Folder picking for custom music.** The original shelled out to a native
  dialog  `NSOpenPanel` on macOS, `zenity` on Linux  and had no Windows
  branch at all, so on Windows the feature could not be reached. This port
  opens the real system folder picker on every desktop platform; see "Custom
  music" above. Everything else about custom music (the extensions, the sort
  order, the 500-file cap, the fallback to the bundled track,
  play-once-then-advance) is the original's.

## Project layout

```
src/
  main.rs     raylib 5.5 handle-API loop; input sampling; screen flow
  audio.rs    background music loop and sound effects (optional device)
  backgrounds.rs per-level background photos (optional; degrades to plain)
  config.rs   dimensions, timing, scoring, gravity ramp, palette + HSV math
  skins.rs    the twelve piece skins: pure colour recipes and block drawing
  pieces.rs   flat shape tables, SRS kick tables, 7-bag
  board.rs    Grid: placement, clearing, T-spin corners, top-out
  game.rs     Game: gravity, DAS/ARR, lock delay, scoring, modes, events
  render.rs   3D gloss blocks, board, side panel, menus, banners, fit/present
  scores.rs   per-mode records with three-letter initials; the entry field
  effects.rs  trail / gesture / dust / explosion particle systems
             plus the Matrix rain's hand-drawn katakana glyph table
  keys.rs     keycode <-> name table (a checked stand-in for raylib's enum)
  pad.rs      controller bindings, SDL-compatible values, capture + slots
  music_dir.rs folder scanning and playlist rules
  folderpick.rs the OS folder dialog (COM on Windows, panel/zenity elsewhere)
  settings.rs persisted settings.json (serde)
build.rs     compiles ICON1.ico into the exe's resource section (.rsrc)
tests/
  icon.rs     the linked binary really carries the icon, read from the .exe
assets/
  tetris-blocks.TTF, music and effect OGGs (copied from the original)
  backgrounds/N.jpg  per-level background photos
  ICON1.ico        the window icon (embedded at build time, see above)
```

## Testing

`cargo test` runs 394 tests across the rules layer, settings serialization,
controller bindings, custom-music folder scanning (including real folders
created and removed by the tests), the piece skins, the colour palettes, the
level-up drain, the Matrix rain glyph table and
automated live play-throughs  no window or assets required. Three of
them drive the real spawn→steer→drop→lock→clear loop with a bot for hundreds
of pieces in every mode, which is how "it actually plays" is verified
headlessly. The bot rotates and scores placements with the standard El-Tetris
heuristic (aggregate height, buried holes, surface bumpiness, completed rows);
scoring only "clears, then fewest holes" ties on an open board, which leaves
the bot stacking one tower and topping out before it clears anything.

Most of the drain's tests exist because of a sign error, not because the
arithmetic was hard. `fx_keep` first returned `1.0 - drain_phase(t)`, which is
*zero at the peak*  the drain vanished at exactly the moment it was supposed to
be strongest  and it passed every test that sampled a non-peak frame, because a
triangle sampled anywhere except its top still has a non-trivial value. The tests
now sample the peak explicitly and assert the triangle's shape separately from
what the effect does with it, so the two can no longer be confused.

The rendering and input layers are deliberately thin so all gameplay logic
stays testable. `TF_SMOKE=1` exercises the render loop itself and writes
`smoke.png` plus a `smoke_grid.txt` dump of the exact board.

### Verifying visual changes without looking at the screen

`TakeScreenshot` is unreliable on a HiDPI display: it scales the capture to
1.896× the render size (a 1920×1080 window yields a 3640×2047 PNG) and anchors
the actual framebuffer bottom-left in the larger canvas, leaving the remainder
black. A single screenshot therefore cannot be trusted for layout  a mostly
black frame is the capture, not the game.

What does work:

- **Diff two captures.** Toggle the feature via an env var, render both, and
  compare pixel counts. Non-zero difference proves the feature reached the
  framebuffer. This is how the background images were confirmed to render
  (106k pixels of real image content, mean luminance 50.7).
- **Compare against `smoke_grid.txt`**, which is exact ground truth for the
  board  row/col from the top, no scaling or capture artefacts.
- **For the scale-to-fit path**, assert on `render::fit` directly and use
  `TF_WINSIZE` to confirm the geometry numerically, rather than eyeballing
  band-by-band luminance profiles.




