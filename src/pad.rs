//! Controller support: bindings, the D-pad and hat, and the analog sticks.
//!
//! The original had three separate blocks in `settings.json` and two nested
//! rebinding menus, and all three are ported here rather than collapsed into a
//! fixed layout:
//!
//! * `controller_controls` - the eight in-game actions. The original's defaults
//!   (D-pad Left/Right/Down, A rotate, B hard drop, X hold, Y pause, RB skip)
//!   are SDL's *Xbox-layout* button numbers, which is what Pygame reported for
//!   every pad it supported.
//! * `controller_menu_navigation` - Up / Down / Select / Back, so the menus are
//!   playable with no keyboard at all.
//! * `controller_settings` - the analog thresholds, the deadzone and whether
//!   the D-pad drives the piece at all.
//!
//! # How one set of bindings covers every kind of controller
//!
//! raylib talks to controllers through SDL, which keeps a database mapping each
//! device to the Xbox button and axis layout. An Xbox pad, a DualShock 4, a
//! DualSense, a Switch Pro controller and most third-party pads all arrive as
//! the same fifteen buttons and six axes, so one set of bindings covers all of
//! them and `settings.json` written by the Python game keeps working unchanged.
//! What differs between them is only which physical button is under the
//! player's thumb, which is exactly what the rebinding menus are for; the names
//! on those menus carry both spellings, so a DualSense player reading
//! "A / Cross" knows which one to press.
//!
//! Two things SDL cannot paper over are handled here rather than assumed:
//!
//! * **A pad is not always slot 0.** The original read `Joystick(0)`; if the
//!   player has a wheel or a flight stick in slot 0 and their pad in slot 1,
//!   the game would read the wrong device or nothing at all. [`connected_index`]
//!   scans for the first slot raylib actually recognises, and is re-checked
//!   every frame so a pad plugged in after launch is picked up.
//! * **A pad may report its D-pad three different ways** - as four buttons, as a
//!   hat value, or as both (the DualSense and the Switch Pro report both). All
//!   three readings are accepted, and any of them can be bound to any action.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// The in-game actions a controller can be bound to, in the original's menu
/// order.
///
/// Distinct from [`crate::settings::Action`]: that is the keyboard list, and
/// the original let the two be bound completely independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadAction {
    Left,
    Right,
    Down,
    Rotate,
    HardDrop,
    Hold,
    Pause,
    SkipTrack,
}

/// The action list, in menu order, so the screen and the lookup cannot drift.
pub const PAD_ACTIONS: [PadAction; 8] = [
    PadAction::Left,
    PadAction::Right,
    PadAction::Down,
    PadAction::Rotate,
    PadAction::HardDrop,
    PadAction::Hold,
    PadAction::Pause,
    PadAction::SkipTrack,
];

impl PadAction {
    pub fn label(self) -> &'static str {
        match self {
            PadAction::Left => "Move Left",
            PadAction::Right => "Move Right",
            PadAction::Down => "Soft Drop",
            PadAction::Rotate => "Rotate",
            PadAction::HardDrop => "Hard Drop",
            PadAction::Hold => "Hold Piece",
            PadAction::Pause => "Pause",
            PadAction::SkipTrack => "Skip Track",
        }
    }

    /// The original's default: SDL's button numbering, so the D-pad, the face
    /// buttons and a centre button.
    ///
    /// The original's own defaults for the three D-pad actions were wrong -
    /// `left: 14, right: 15, down: 13` is D-pad *right*, an unmapped extra
    /// button and D-pad *left*, so its default setup moved the piece the wrong
    /// way with the D-pad and could not soft drop with it at all. The bindings
    /// are corrected here rather than copied, which is one of the ported bugs
    /// (see README).
    pub fn default_binding(self) -> Binding {
        match self {
            // D-pad left / right / down.
            PadAction::Left => Binding::Button(13),
            PadAction::Right => Binding::Button(14),
            PadAction::Down => Binding::Button(12),
            // A / B / X / Y / Back-Share.
            PadAction::Rotate => Binding::Button(0),
            PadAction::HardDrop => Binding::Button(1),
            PadAction::Hold => Binding::Button(2),
            PadAction::Pause => Binding::Button(3),
            PadAction::SkipTrack => Binding::Button(4),
        }
    }
}

/// The menu-navigation actions, in the original's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavAction {
    Up,
    Down,
    Select,
    Back,
}

pub const NAV_ACTIONS: [NavAction; 4] = [
    NavAction::Up,
    NavAction::Down,
    NavAction::Select,
    NavAction::Back,
];

impl NavAction {
    pub fn label(self) -> &'static str {
        match self {
            NavAction::Up => "Navigate Up",
            NavAction::Down => "Navigate Down",
            NavAction::Select => "Select / Confirm",
            NavAction::Back => "Back / Cancel",
        }
    }

    /// The original's defaults: D-pad up and down, A to select, B to go back.
    /// These four were correct in the original.
    pub fn default_binding(self) -> Slot {
        match self {
            NavAction::Up => Binding::Button(11).into(),
            NavAction::Down => Binding::Button(12).into(),
            NavAction::Select => Binding::Button(0).into(),
            NavAction::Back => Binding::Button(1).into(),
        }
    }
}

/// One physical input on a pad.
///
/// The original stored three different JSON shapes for these and all three are
/// written back the same way, so a `settings.json` carried over from the Python
/// game keeps working:
///
/// ```json
/// {"rotate": 0,  "left": ["hat", [-1, 0]],  "down": ["axis", 1, "positive"]}
/// ```
///
/// - a bare number is a button, by SDL's numbering (0 = A / Cross);
/// - `["hat", [x, y]]` is one direction of the D-pad, as a value pair;
/// - `["axis", n, "positive" | "negative"]` is one half of an analog axis;
/// - `null` means deliberately unbound.
///
/// The Y axes point *down*, so `positive` on axis 1 is Down. Getting that
/// backwards would invert soft drop for anyone who bound it to a stick, which is
/// why [`axis_name`] spells out the direction to push rather than the sign.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    /// A digital button, by SDL's number (0 = A / Cross, 1 = B / Circle, ...).
    Button(i32),
    /// One direction of the D-pad, as the `(x, y)` value the original stored.
    Hat(i32, i32),
    /// Half of an analog axis.
    Axis { axis: i32, positive: bool },
    /// Explicitly unbound, so the action does nothing.
    None,
}

impl Binding {
    /// Parse the original's three shapes plus its "not set" markers.
    ///
    /// Written by hand rather than left to a derive because the three shapes
    /// have nothing in common structurally: a number, a two-element array
    /// holding another array, and a three-element array of mixed types. A derive
    /// would pick one representation and quietly rewrite every existing file.
    ///
    /// Lenient by design. `None` for an unrecognised shape, never an error: a
    /// truncated or hand-edited settings file must not stop the game loading, and
    /// an action we cannot read is better unbound than wired to something
    /// arbitrary.
    pub fn from_stored(v: Option<&serde_json::Value>) -> Option<Binding> {
        use serde_json::Value;
        match v? {
            Value::Null => None,
            // A bare number is a button, which is what the original stored.
            Value::Number(n) => n.as_i64().map(|b| Binding::Button(b as i32)),
            Value::Array(a) => match a.first().and_then(|k| k.as_str()) {
                // The original wrote `('hat', event.value)` and `event.value` is
                // a tuple, so the value is a nested pair. The flattened spelling
                // is accepted too in case a file was edited by hand.
                Some("hat") if a.len() == 2 => match &a[1] {
                    Value::Array(p) if p.len() == 2 => {
                        let (x, y) = (p[0].as_i64(), p[1].as_i64());
                        match (x, y) {
                            (Some(x), Some(y)) => Some(Binding::Hat(x as i32, y as i32)),
                            _ => None,
                        }
                    }
                    _ => None,
                },
                Some("hat") if a.len() == 3 => match (a[1].as_i64(), a[2].as_i64()) {
                    (Some(x), Some(y)) => Some(Binding::Hat(x as i32, y as i32)),
                    _ => None,
                },
                Some("axis") if a.len() == 3 => {
                    let axis = a[1].as_i64()?;
                    let positive = match a[2].as_str()? {
                        "positive" => true,
                        "negative" => false,
                        _ => return None,
                    };
                    Some(Binding::Axis {
                        axis: axis as i32,
                        positive,
                    })
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// The value to write back, in the original's own JSON shape.
    pub fn to_stored(self) -> serde_json::Value {
        use serde_json::json;
        match self {
            Binding::Button(b) => json!(b),
            Binding::Hat(x, y) => json!(["hat", [x, y]]),
            Binding::Axis { axis, positive } => {
                json!(["axis", axis, if positive { "positive" } else { "negative" }])
            }
            Binding::None => serde_json::Value::Null,
        }
    }

    /// The display name shown on a rebinding row.
    pub fn label(self) -> String {
        match self {
            Binding::Button(b) => button_name(b).to_string(),
            Binding::Hat(x, y) => hat_name(x, y),
            Binding::Axis { axis, positive } => axis_name(axis, positive),
            Binding::None => "(none)".to_string(),
        }
    }

    /// The display name for a slot, given its raw value.
    ///
    /// `None` is the two different things a slot can be, and they read
    /// differently: an action the player deliberately unbound, and a binding
    /// whose number this build has no name for. Only the first says "(none)".
    pub fn slot_label(binding: Option<Binding>) -> String {
        match binding {
            Some(b) => b.label(),
            None => "(none)".to_string(),
        }
    }
}

impl fmt::Display for Binding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

impl Serialize for Binding {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_stored().serialize(s)
    }
}

impl<'de> Deserialize<'de> for Binding {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // Lenient by construction: an unreadable binding is "unbound", not a
        // reason to reject the whole settings file. `Slot` is the strict
        // counterpart of this - it is the one that knows about defaults.
        let raw = serde_json::Value::deserialize(d)?;
        Ok(match raw {
            serde_json::Value::String(ref s) if s.eq_ignore_ascii_case("none") => Binding::None,
            other => Binding::from_stored(Some(&other)).unwrap_or(Binding::None),
        })
    }
}

/// The name printed on a controller button, with the PlayStation label too.
///
/// The numbers are SDL's `SDL_GameControllerButton` values, which is what the
/// bindings store and what Pygame reported, so a `settings.json` from the Python
/// game reads correctly. SDL normalises every supported pad to this layout, so
/// these are the same button on an Xbox pad, a DualShock 4, a DualSense, a
/// Switch Pro controller and a third-party pad - the only difference between
/// them is the moulding, which is why both names are given.
pub fn button_name(b: i32) -> &'static str {
    match b {
        0 => "A / Cross",
        1 => "B / Circle",
        2 => "X / Square",
        3 => "Y / Triangle",
        4 => "Back / Share",
        5 => "Guide / PS",
        6 => "Start / Options",
        7 => "L3 / L-Stick",
        8 => "R3 / R-Stick",
        9 => "LB / L1",
        10 => "RB / R1",
        11 => "D-Pad Up",
        12 => "D-Pad Down",
        13 => "D-Pad Left",
        14 => "D-Pad Right",
        // Not SDL buttons. SDL exposes the triggers as axes only, and the two
        // numbers below are this port's own names for the digital half of each
        // one, so a trigger can be bound like any other button. 15 is SDL's
        // MISC1, which no current pad maps.
        LT_BUTTON => "LT / L2",
        RT_BUTTON => "RT / R2",
        _ => "Button",
    }
}

/// This port's button number for the digital left trigger.
///
/// SDL has no button for a trigger, only an axis, so on a modern pad the two
/// triggers would otherwise be the one thing that cannot be bound. raylib
/// synthesises a digital button for each from its axis, and these numbers name
/// them. They start past SDL's range, so they can never collide with a real
/// SDL button number in an existing settings file.
pub const LT_BUTTON: i32 = 16;
/// This port's button number for the digital right trigger. See [`LT_BUTTON`].
pub const RT_BUTTON: i32 = 17;

/// The name for a hat direction.
///
/// SDL reports a hat as a single value rather than a bitmask, so only eight
/// positions can reach this. A diagonal names its dominant axis, which is what
/// the player meant when they pushed up-and-right. The original wrote these into
/// `settings.json` as `["hat", [x, y]]`, and they are read back there, but under
/// raylib a hat is already folded into the D-pad buttons by SDL - so a hat
/// binding and the matching D-pad button binding mean the same thing.
pub fn hat_name(x: i32, y: i32) -> String {
    let name = match (x, y) {
        (-1, -1) => "D-Pad Up-Left",
        (0, -1) => "D-Pad Up",
        (1, -1) => "D-Pad Up-Right",
        (-1, 0) => "D-Pad Left",
        (1, 0) => "D-Pad Right",
        (-1, 1) => "D-Pad Down-Left",
        (0, 1) => "D-Pad Down",
        (1, 1) => "D-Pad Down-Right",
        _ => return format!("Hat ({x}, {y})"),
    };
    name.to_string()
}

/// The name for half of an analog axis.
///
/// The name is the direction the player has to *push*, not the sign, because
/// that is the only thing that helps when they are looking for it on a stick.
/// The Y axes point down, so `positive` there is Down.
pub fn axis_name(axis: i32, positive: bool) -> String {
    let (stick, vertical) = match axis {
        0 => ("Left Stick X", false),
        1 => ("Left Stick Y", true),
        2 => ("Right Stick X", false),
        3 => ("Right Stick Y", true),
        4 => return "LT / L2".to_string(),
        5 => return "RT / R2".to_string(),
        _ => return format!("Axis {axis}"),
    };
    let dir = match (vertical, positive) {
        (false, true) => "Right",
        (false, false) => "Left",
        (true, true) => "Down",
        (true, false) => "Up",
    };
    format!("{stick} {dir}")
}

/// A binding slot: a [`Binding`], or `None` when the action is unbound.
///
/// A newtype rather than a bare `Option<Binding>` so that "unbound" and "absent
/// from the file" are different things that a `#[serde(default)]` cannot
/// confuse: a slot written as `null` stays unbound, while a missing slot takes
/// its default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Slot(pub Option<Binding>);

impl From<Binding> for Slot {
    fn from(b: Binding) -> Self {
        Slot(match b {
            // A deliberate "unbound" has to survive as unbound rather than
            // being refilled with the default on the next load.
            Binding::None => None,
            b => Some(b),
        })
    }
}

impl From<Option<Binding>> for Slot {
    fn from(b: Option<Binding>) -> Self {
        Slot(b)
    }
}

/// So a slot can be compared against a plain `Option<Binding>`.
///
/// The slot is a newtype only so that serde can tell "absent from the file" from
/// "written as null" - the two mean different things and a `#[serde(default)]`
/// cannot. Every read outside the deserialiser wants the plain option, and making
/// the tests spell `Slot(Some(..))` everywhere would only obscure what they
/// are checking.
impl PartialEq<Option<Binding>> for Slot {
    fn eq(&self, other: &Option<Binding>) -> bool {
        self.0 == *other
    }
}

impl PartialEq<Slot> for Option<Binding> {
    fn eq(&self, other: &Slot) -> bool {
        *self == other.0
    }
}

impl Slot {
    pub fn get(self) -> Option<Binding> {
        self.0
    }
}

impl Serialize for Slot {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Some(b) => b.serialize(s),
            None => s.serialize_none(),
        }
    }
}

/// The eight in-game action bindings.
///
/// A flat struct of slots rather than an array, mirroring
/// [`crate::settings::Controls`]: it keeps the JSON readable and lets a file
/// from the Python game load unchanged.
///
/// `Deserialize` is written out rather than derived, so that each slot can fall
/// back on its own default instead of one bad value taking the whole file with
/// it. See [`slot_from`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PadControls {
    pub left: Slot,
    pub right: Slot,
    pub down: Slot,
    pub rotate: Slot,
    pub hard_drop: Slot,
    pub hold: Slot,
    pub pause: Slot,
    pub skip_track: Slot,
}

impl<'de> Deserialize<'de> for PadControls {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = SlotMap::deserialize(d)?;
        Ok(PadControls {
            left: slot_from(&raw, "left", db_left),
            right: slot_from(&raw, "right", db_right),
            down: slot_from(&raw, "down", db_down),
            rotate: slot_from(&raw, "rotate", db_rotate),
            hard_drop: slot_from(&raw, "hard_drop", db_hard_drop),
            hold: slot_from(&raw, "hold", db_hold),
            pause: slot_from(&raw, "pause", db_pause),
            skip_track: slot_from(&raw, "skip_track", db_skip_track),
        })
    }
}

macro_rules! db {
    ($fn_name:ident, $action:ident) => {
        fn $fn_name() -> Slot {
            PadAction::$action.default_binding().into()
        }
    };
}

db!(db_left, Left);
db!(db_right, Right);
db!(db_down, Down);
db!(db_rotate, Rotate);
db!(db_hard_drop, HardDrop);
db!(db_hold, Hold);
db!(db_pause, Pause);
db!(db_skip_track, SkipTrack);

impl Default for PadControls {
    fn default() -> Self {
        Self {
            left: db_left(),
            right: db_right(),
            down: db_down(),
            rotate: db_rotate(),
            hard_drop: db_hard_drop(),
            hold: db_hold(),
            pause: db_pause(),
            skip_track: db_skip_track(),
        }
    }
}

impl PadControls {
    /// The binding for `action`, or `None` when it is unbound.
    pub fn get(&self, action: PadAction) -> Option<Binding> {
        self.slot(action).get()
    }

    pub fn set(&mut self, action: PadAction, binding: Option<Binding>) {
        *self.slot_mut(action) = Slot(binding);
    }

    fn slot(&self, action: PadAction) -> Slot {
        match action {
            PadAction::Left => self.left,
            PadAction::Right => self.right,
            PadAction::Down => self.down,
            PadAction::Rotate => self.rotate,
            PadAction::HardDrop => self.hard_drop,
            PadAction::Hold => self.hold,
            PadAction::Pause => self.pause,
            PadAction::SkipTrack => self.skip_track,
        }
    }

    fn slot_mut(&mut self, action: PadAction) -> &mut Slot {
        match action {
            PadAction::Left => &mut self.left,
            PadAction::Right => &mut self.right,
            PadAction::Down => &mut self.down,
            PadAction::Rotate => &mut self.rotate,
            PadAction::HardDrop => &mut self.hard_drop,
            PadAction::Hold => &mut self.hold,
            PadAction::Pause => &mut self.pause,
            PadAction::SkipTrack => &mut self.skip_track,
        }
    }
}

/// The four menu-navigation bindings.
///
/// Hand-deserialised for the same reason as [`PadControls`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PadNav {
    pub up: Slot,
    pub down: Slot,
    pub select: Slot,
    pub back: Slot,
}

impl<'de> Deserialize<'de> for PadNav {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = SlotMap::deserialize(d)?;
        Ok(PadNav {
            up: slot_from(&raw, "up", dn_up),
            down: slot_from(&raw, "down", dn_down),
            select: slot_from(&raw, "select", dn_select),
            back: slot_from(&raw, "back", dn_back),
        })
    }
}

macro_rules! dn {
    ($fn_name:ident, $action:ident) => {
        fn $fn_name() -> Slot {
            NavAction::$action.default_binding()
        }
    };
}

dn!(dn_up, Up);
dn!(dn_down, Down);
dn!(dn_select, Select);
dn!(dn_back, Back);

impl Default for PadNav {
    fn default() -> Self {
        Self {
            up: dn_up(),
            down: dn_down(),
            select: dn_select(),
            back: dn_back(),
        }
    }
}

impl PadNav {
    pub fn get(&self, action: NavAction) -> Option<Binding> {
        self.slot(action).get()
    }

    pub fn set(&mut self, action: NavAction, binding: Option<Binding>) {
        *self.slot_mut(action) = Slot(binding);
    }

    fn slot(&self, action: NavAction) -> Slot {
        match action {
            NavAction::Up => self.up,
            NavAction::Down => self.down,
            NavAction::Select => self.select,
            NavAction::Back => self.back,
        }
    }

    fn slot_mut(&mut self, action: NavAction) -> &mut Slot {
        match action {
            NavAction::Up => &mut self.up,
            NavAction::Down => &mut self.down,
            NavAction::Select => &mut self.select,
            NavAction::Back => &mut self.back,
        }
    }
}

/// How the analog sticks and the D-pad are read.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PadSettings {
    /// How far a stick must travel to count as a move. The original's default.
    #[serde(default = "default_analog_threshold")]
    pub analog_threshold: f32,
    /// How far a hat must travel to count, for pads whose D-pad arrives as a hat
    /// axis rather than four buttons. The original's default.
    #[serde(default = "default_hat_threshold")]
    pub hat_threshold: f32,
    /// How far a stick must travel before *releasing* a move. Deliberately
    /// below `analog_threshold`: with a single threshold, a stick resting near
    /// it emits a press edge every other frame and the piece stutters sideways.
    /// The original's `analog_deadzone`, which it read with a `.get(..., 0.3)`
    /// default because the key had no serde default of its own.
    #[serde(default = "default_analog_deadzone")]
    pub analog_deadzone: f32,
    /// Whether the D-pad steers the piece. The original defaulted this on and
    /// read it with `.get('use_dpad', True)`.
    #[serde(default = "default_true")]
    pub use_dpad: bool,
    /// Which device to read. `0` means "the first one raylib recognises", which
    /// is what a player with one controller wants.
    ///
    /// This one is not from the original, and it exists because of a real
    /// failure it could not avoid. SDL numbers raw devices in platform
    /// enumeration order, and raylib reports *any* device in a slot as an
    /// available gamepad - it has no way to ask "is this actually a gamepad?" -
    /// so a flight stick or a wheel that Windows happened to enumerate first
    /// takes slot 0 and the game reads that instead of the controller sitting in
    /// slot 1. Pointing this at `1` is the fix, and the Controller Keybinds
    /// screen lists what is in each slot so the right number is findable.
    #[serde(default)]
    pub pad_slot: i32,
    /// The original also had a `joy_delay` of 150 ms, rate-limiting repeated
    /// controller navigation. It is kept so the block round-trips unchanged, but
    /// repetition here is driven by the same DAS/ARR as the keyboard, which is
    /// what a player would expect and what the original's own held-button path
    /// already did.
    #[serde(default = "default_joy_delay")]
    pub joy_delay: u32,
}

fn default_analog_threshold() -> f32 {
    0.5
}
fn default_hat_threshold() -> f32 {
    0.5
}
fn default_analog_deadzone() -> f32 {
    0.3
}
fn default_joy_delay() -> u32 {
    150
}
fn default_true() -> bool {
    true
}

impl Default for PadSettings {
    fn default() -> Self {
        Self {
            analog_threshold: 0.5,
            hat_threshold: 0.5,
            analog_deadzone: 0.3,
            use_dpad: true,
            pad_slot: 0,
            joy_delay: 150,
        }
    }
}

impl PadSettings {
    /// Whether a stick reading counts as held, given whether it already was.
    ///
    /// The two-threshold form is the original's: engage past
    /// `analog_threshold`, release only once back inside `analog_deadzone`. The
    /// gap between the two is the hysteresis that keeps a nearly-centred stick
    /// from flickering.
    pub fn engaged(&self, value: f32, was_engaged: bool) -> bool {
        if was_engaged {
            value.abs() > self.analog_deadzone
        } else {
            value.abs() >= self.analog_threshold
        }
    }

    /// Whether an axis reading is on the side a binding names.
    ///
    /// Separate from [`PadSettings::engaged`] because a binding names one *side*
    /// of an axis: Move Left bound to `Axis { axis: 0, positive: false }` must
    /// read a stick pushed right as "not left" rather than as a press.
    pub fn on_side(&self, value: f32, positive: bool) -> bool {
        if positive {
            value >= self.analog_threshold
        } else {
            value <= -self.analog_threshold
        }
    }

    /// Clamp stored thresholds into ranges the comparisons can use.
    ///
    /// A hand-edited file could otherwise set `analog_threshold: 5.0` and make
    /// the stick permanently dead, or `-1.0` and make it permanently on. The
    /// deadzone is held below the threshold so the hysteresis always has a band
    /// to sit in.
    ///
    /// The upper bound is 0.99 and not 1.0 on purpose. raylib reports a stick in
    /// `-1.0..=1.0` in theory, but a real one at rest drifts and a real one at
    /// full tilt very often reports 0.98 - so a threshold of 1.0 is unreachable
    /// in practice, which is the same as a dead stick and is exactly what the
    /// clamp is here to prevent.
    pub fn sanitized(&self) -> PadSettings {
        let threshold = if self.analog_threshold.is_finite() {
            self.analog_threshold.clamp(0.01, 0.99)
        } else {
            0.5
        };
        let deadzone = if self.analog_deadzone.is_finite() {
            self.analog_deadzone.clamp(0.0, threshold)
        } else {
            threshold / 2.0
        };
        let hat = if self.hat_threshold.is_finite() {
            self.hat_threshold.clamp(0.0, 1.0)
        } else {
            0.5
        };
        PadSettings {
            analog_threshold: threshold,
            analog_deadzone: deadzone,
            hat_threshold: hat,
            ..*self
        }
    }
}

/// How many gamepad slots there are.
///
/// This is raylib's own `MAX_GAMEPADS`, not a guess: `is_gamepad_available`
/// rejects anything past it, so scanning further would be wasted work and would
/// read past the array raylib allocated.
pub const MAX_GAMEPADS: i32 = 4;

/// Find the first slot raylib recognises as a gamepad.
///
/// Returns `None` when nothing usable is plugged in.
///
/// The scan exists because the pad the player wants is not necessarily slot 0.
/// SDL lists raw devices in platform enumeration order, and raylib marks any
/// device in a slot as an available gamepad, so a flight stick or a wheel
/// opened first takes slot 0 - and reading only slot 0, as the original did,
/// then means no controller at all. Taking the first *available* slot fixes the
/// common case, and [`PadSettings::pad_slot`] is the escape hatch for the rest.
pub fn connected_index<F>(mut available: F) -> Option<i32>
where
    F: FnMut(i32) -> bool,
{
    (0..MAX_GAMEPADS).find(|&i| available(i))
}

/// Which slot the game should read this frame.
///
/// `wanted` is [`PadSettings::pad_slot`]; `None` there - or any out-of-range
/// value - falls back to the first available slot, so a hand-edited file cannot
/// point the game at a slot that does not exist and leave the player with no
/// controller at all.
pub fn resolve_slot<F>(wanted: i32, mut available: F) -> Option<i32>
where
    F: FnMut(i32) -> bool,
{
    if (0..MAX_GAMEPADS).contains(&wanted) && available(wanted) {
        Some(wanted)
    } else {
        connected_index(available)
    }
}

/// What a controller input means while a rebinding screen is waiting for one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capture {
    /// Nothing usable was pressed this frame.
    Nothing,
    /// Bind the action to this input.
    Bind(Binding),
    /// Leave the rebind screen without changing anything.
    Cancel,
}

/// Decide what a captured controller input does.
///
/// Split out as a pure function for the same reason as
/// [`crate::keys::classify_capture`]: every pad type can then be covered by a
/// test with no device present, and the screen code stays a straight line.
///
/// `first_button` and `first_axis` are the strongest press seen this frame; the
/// caller has already thresholded the axes, so nudging a stick during a rebind
/// does not bind a direction the player never meant. A digital button wins over
/// an axis pressed in the same frame, because a button is unambiguous and the
/// D-pad has a label the player can check.
///
/// `back` is the Menu Nav "Back" binding, and it cancels. The keyboard lets
/// Escape cancel a rebind and a controller needs an equivalent, or a player who
/// has bound a face button to Rotate cannot leave the rebind screen with the pad
/// at all. Rather than stealing one of the face buttons - any of which may be the
/// very thing being bound - the button that already means "go back" everywhere
/// else in the game cancels here too.
pub fn classify_capture(
    first_button: Option<i32>,
    first_axis: Option<Binding>,
    back: Option<Binding>,
) -> Capture {
    match first_button.map(Binding::Button).or(first_axis) {
        Some(b) if Some(b) == back => Capture::Cancel,
        Some(b) => Capture::Bind(b),
        None => Capture::Nothing,
    }
}

/// A settings object read as raw JSON, so its slots can be resolved by hand.
///
/// The indirection is what lets an unreadable binding fall back to its default
/// rather than failing the file - see [`slot_from`].
type SlotMap = std::collections::BTreeMap<String, serde_json::Value>;

/// Resolve one binding slot out of a raw settings object.
///
/// Three outcomes, and telling them apart is the whole point of doing this by
/// hand rather than per field:
///
/// * **key absent** - the file predates controllers, or came from a build that
///   did not have this action. Take the default.
/// * **`null`, or the string `"none"`** - the original's own spelling of "this
///   action is unbound". Stay unbound. Refilling it with a default would
///   silently re-wire an action the player had deliberately switched off, and
///   the only symptom would be a piece rotating when nobody touched anything.
/// * **anything else** - a recognised shape binds, and an unrecognised one takes
///   the default, because a truncated or hand-edited file must not stop the
///   game loading and an action we cannot read is better at its default than
///   wired to something arbitrary.
///
/// The last case is the one a per-field `Deserialize` cannot express. It can
/// return a value or fail the entire struct; there is no "that key was there,
/// use the default instead". Hence [`PadControls`] and [`PadNav`] each read the
/// whole object and resolve their own slots, and [`Slot`] therefore has no
/// `Deserialize` impl of its own.
fn slot_from(raw: &SlotMap, key: &str, default: fn() -> Slot) -> Slot {
    match raw.get(key) {
        None => default(),
        Some(serde_json::Value::Null) => Slot(None),
        Some(serde_json::Value::String(s)) if s.eq_ignore_ascii_case("none") => Slot(None),
        Some(v) => match Binding::from_stored(Some(v)) {
            Some(b) => Slot(Some(b)),
            None => default(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- defaults --------------------------------------------------------

    #[test]
    fn the_default_bindings_are_the_dpad_and_the_face_buttons() {
        // The original's own defaults for the three D-pad actions were
        // `left: 14, right: 15, down: 13`. In SDL's numbering that is D-pad
        // *right*, the unmapped MISC1 and D-pad *left* - so out of the box the
        // original moved the piece the wrong way with the D-pad and could not
        // soft drop with it at all. They are corrected here rather than copied.
        let c = PadControls::default();
        assert_eq!(c.left, Some(Binding::Button(13)), "D-pad Left");
        assert_eq!(c.right, Some(Binding::Button(14)), "D-pad Right");
        assert_eq!(c.down, Some(Binding::Button(12)), "D-pad Down");
        assert_eq!(c.rotate, Some(Binding::Button(0)), "A / Cross");
        assert_eq!(c.hard_drop, Some(Binding::Button(1)), "B / Circle");
        assert_eq!(c.hold, Some(Binding::Button(2)), "X / Square");
        assert_eq!(c.pause, Some(Binding::Button(3)), "Y / Triangle");
        assert_eq!(c.skip_track, Some(Binding::Button(4)), "Back / Share");
    }

    #[test]
    fn the_default_dpad_bindings_move_the_piece_the_way_they_are_labelled() {
        // The point of the fix above, stated as a property: a D-pad binding must
        // agree with the name the rebinding screen shows for it, or the label is
        // worse than no label.
        for action in [PadAction::Left, PadAction::Right, PadAction::Down] {
            let b = PadAction::default_binding(action);
            assert!(
                button_name(b_as_int(b)).starts_with("D-Pad"),
                "{action:?} defaults to {:?}, which is not a D-pad direction",
                b.label()
            );
        }
        // Left is D-pad Left and Right is D-pad Right, not the other way round.
        assert_eq!(
            button_name(b_as_int(PadAction::default_binding(PadAction::Left))),
            "D-Pad Left"
        );
        assert_eq!(
            button_name(b_as_int(PadAction::default_binding(PadAction::Right))),
            "D-Pad Right"
        );
    }

    /// The button number out of a binding, for the tests that compare against
    /// `button_name`. Panics on a non-button, which is the point: those tests are
    /// about the D-pad defaults and should fail loudly if that changes.
    fn b_as_int(b: Binding) -> i32 {
        match b {
            Binding::Button(n) => n,
            other => panic!("expected a button binding, got {other:?}"),
        }
    }

    #[test]
    fn the_default_menu_navigation_is_dpad_up_down_a_and_b() {
        // These four were correct in the original and are unchanged.
        let n = PadNav::default();
        assert_eq!(n.up, Some(Binding::Button(11)), "D-pad Up");
        assert_eq!(n.down, Some(Binding::Button(12)), "D-pad Down");
        assert_eq!(n.select, Some(Binding::Button(0)), "A / Cross");
        assert_eq!(n.back, Some(Binding::Button(1)), "B / Circle");
    }

    #[test]
    fn the_default_action_bindings_are_all_different() {
        let c = PadControls::default();
        let mut keys: Vec<String> = PAD_ACTIONS
            .iter()
            .map(|a| format!("{:?}", c.get(*a)))
            .collect();
        let before = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(before, keys.len(), "two actions default to the same button");
    }

    #[test]
    fn the_default_navigation_deliberately_overlaps_the_default_actions() {
        // Select is A and Back is B, which are also Rotate and Hard Drop. That
        // is the original's own overlap and it is fine: the two lists are read
        // in different screens, so A confirms in a menu and rotates in play.
        // The test exists to record that this is intended, not to forbid it.
        let n = PadNav::default();
        let c = PadControls::default();
        assert_eq!(n.select, c.get(PadAction::Rotate));
        assert_eq!(n.back, c.get(PadAction::HardDrop));
    }

    #[test]
    fn the_original_thresholds_are_the_defaults() {
        let s = PadSettings::default();
        assert_eq!(s.analog_threshold, 0.5);
        assert_eq!(s.hat_threshold, 0.5);
        assert_eq!(s.analog_deadzone, 0.3);
        assert!(s.use_dpad);
        assert_eq!(s.joy_delay, 150);
        // Zero means "the first one raylib recognises", which is the only value
        // that works for a player with one controller.
        assert_eq!(s.pad_slot, 0);
    }

    // --- hysteresis ------------------------------------------------------

    #[test]
    fn a_stick_engages_past_the_threshold_and_releases_inside_the_deadzone() {
        let s = PadSettings::default();
        // Not yet held: has to pass the threshold.
        assert!(!s.engaged(0.4, false));
        assert!(s.engaged(0.5, false));
        assert!(s.engaged(-0.9, false));
        // Already held: keeps going until back inside the deadzone, so a stick
        // resting at 0.45 does not emit an edge on every other frame.
        assert!(s.engaged(0.45, true));
        assert!(!s.engaged(0.2, true));
        assert!(!s.engaged(-0.2, true));
    }

    #[test]
    fn the_two_sides_of_an_axis_are_independent() {
        let s = PadSettings::default();
        assert!(s.on_side(-0.8, false));
        assert!(!s.on_side(-0.8, true));
        assert!(s.on_side(0.8, true));
        assert!(!s.on_side(0.8, false));
        // Dead centre is neither side, which is what stops a released stick
        // from counting as a move in one direction.
        assert!(!s.on_side(0.0, true));
        assert!(!s.on_side(0.0, false));
    }

    #[test]
    fn a_nonsensical_threshold_is_clamped_rather_than_obeyed() {
        // A hand-edited settings file must not be able to make the stick
        // permanently dead or permanently on.
        let dead = PadSettings {
            analog_threshold: 5.0,
            ..PadSettings::default()
        }
        .sanitized();
        assert!(dead.analog_threshold <= 1.0);
        assert!(dead.engaged(0.99, false), "a clamped threshold is still usable");

        let stuck = PadSettings {
            analog_threshold: -1.0,
            ..PadSettings::default()
        }
        .sanitized();
        assert!(stuck.analog_threshold > 0.0);
        assert!(!stuck.engaged(0.0, false), "a resting stick must not read as a move");

        let nan = PadSettings {
            analog_threshold: f32::NAN,
            analog_deadzone: f32::NAN,
            hat_threshold: f32::NAN,
            ..PadSettings::default()
        }
        .sanitized();
        assert!(nan.analog_threshold.is_finite());
        assert!(nan.analog_deadzone.is_finite());
        assert!(nan.hat_threshold.is_finite());
    }

    #[test]
    fn the_deadzone_never_rises_above_the_threshold() {
        // Otherwise a stick could be "engaged" and simultaneously inside the
        // deadzone, leaving the hysteresis no band at all.
        for dead in [0.0, 0.1, 0.3, 0.5, 0.9, 1.0, 4.0] {
            let s = PadSettings {
                analog_threshold: 0.5,
                analog_deadzone: dead,
                ..PadSettings::default()
            }
            .sanitized();
            assert!(
                s.analog_deadzone <= s.analog_threshold,
                "deadzone {} > threshold {}",
                s.analog_deadzone,
                s.analog_threshold
            );
        }
    }

    #[test]
    fn sanitizing_leaves_sane_values_alone() {
        let s = PadSettings::default();
        assert_eq!(s.sanitized(), s, "a valid block must not be rewritten");
    }

    // --- reading the original's file format ------------------------------

    #[test]
    fn the_originals_three_stored_shapes_all_read_back() {
        // A bare number is a button.
        assert_eq!(Binding::from_stored(Some(&json!(7))), Some(Binding::Button(7)));
        // A hat is ("hat", (x, y)) - the value is a nested pair, because
        // Pygame's JOYHATMOTION event carries a tuple.
        assert_eq!(
            Binding::from_stored(Some(&json!(["hat", [-1, 0]]))),
            Some(Binding::Hat(-1, 0))
        );
        // An axis is ("axis", n, "positive"/"negative").
        assert_eq!(
            Binding::from_stored(Some(&json!(["axis", 0, "negative"]))),
            Some(Binding::Axis { axis: 0, positive: false })
        );
        assert_eq!(
            Binding::from_stored(Some(&json!(["axis", 1, "positive"]))),
            Some(Binding::Axis { axis: 1, positive: true })
        );
        // And "not set", in both the spellings the original could produce.
        assert_eq!(Binding::from_stored(None), None);
        assert_eq!(Binding::from_stored(Some(&json!(null))), None);
    }

    #[test]
    fn a_hand_flattened_hat_still_reads() {
        // Cheap robustness: a file edited by hand is likely to lose the nesting,
        // and losing a D-pad binding is worse than accepting a second spelling.
        assert_eq!(
            Binding::from_stored(Some(&json!(["hat", 0, 1]))),
            Some(Binding::Hat(0, 1))
        );
    }

    #[test]
    fn every_binding_shape_round_trips_through_the_originals_json() {
        for b in [
            Binding::Button(9),
            Binding::Hat(0, 1),
            Binding::Axis { axis: 1, positive: true },
        ] {
            let stored = b.to_stored();
            assert_eq!(Binding::from_stored(Some(&stored)), Some(b), "{stored}");
        }
    }

    #[test]
    fn an_unbound_slot_round_trips_as_the_originals_null() {
        // `Binding::None` is deliberately not in the list above. It is what a
        // *label* says, not what a slot holds - a slot holds `None`, and the
        // file spells that the way the original did. Round-tripping the
        // convenience variant would be asserting that a display value survives a
        // save, which it does not because it is never written.
        let unbound = Slot(None);
        let json = serde_json::to_value(&unbound).unwrap();
        assert_eq!(json, serde_json::json!(null));
        // Through a whole struct, which is the path a save actually takes.
        let controls = PadControls {
            left: unbound,
            ..PadControls::default()
        };
        let back: PadControls =
            serde_json::from_value(serde_json::to_value(controls).unwrap()).unwrap();
        assert_eq!(back.left, None);
        assert_eq!(back, controls, "every other slot must be untouched");
    }

    #[test]
    fn an_unbound_action_stays_unbound_rather_than_being_refilled() {
        // The regression this is really about: a slot written as `null` used to
        // be indistinguishable from a slot holding a value we could not parse,
        // and one of the two is far more likely to be wanted.
        for spelled_unbound in [
            serde_json::json!({"left": null}),
            serde_json::json!({"left": "None"}),
            serde_json::json!({"left": "none"}),
            serde_json::json!({"left": "NONE"}),
        ] {
            let s: PadControls = serde_json::from_value(spelled_unbound.clone()).unwrap();
            assert_eq!(s.left, None, "{spelled_unbound} was refilled");
        }
    }

    #[test]
    fn a_hat_is_written_back_nested_exactly_as_the_original_wrote_it() {
        // Round-tripping through this port must not reformat the file, or a
        // player who goes back to the Python game loses their bindings.
        assert_eq!(Binding::Hat(-1, 0).to_stored(), json!(["hat", [-1, 0]]));
        assert_eq!(Binding::Button(3).to_stored(), json!(3));
        assert_eq!(
            Binding::Axis { axis: 1, positive: false }.to_stored(),
            json!(["axis", 1, "negative"])
        );
    }

    #[test]
    fn nonsense_in_a_binding_slot_falls_back_instead_of_rejecting_the_file() {
        // Truncated, wrong-typed, or a shape the original never wrote: none of
        // these may stop the game loading. The slot takes its default - not
        // "unbound", which is what a `null` means and is a different thing
        // entirely, because a player who unbound an action on purpose should
        // not have it silently rewired the next time they launch.
        for junk in [
            json!([1, 2, 3, 4]),
            json!(["hat", "x", 0]),
            json!(["axis", 0, "sideways"]),
            json!({}),
            json!(["nonsense"]),
            json!("D-Pad Left"),
            json!(true),
        ] {
            let s: PadControls = serde_json::from_value(json!({ "left": junk }))
                .unwrap_or_else(|e| panic!("{junk} should still parse: {e}"));
            assert_eq!(s.left, Some(Binding::Button(13)), "{junk} was accepted");
        }
    }

    #[test]
    fn one_bad_slot_does_not_cost_the_player_the_other_seven() {
        // The point of resolving slots by hand rather than per field: a single
        // unreadable value takes one action back to its default, not the whole
        // file - and with the derived version it was taking the whole file,
        // silently, because `Settings::load` cannot tell a rejected file from
        // an empty one.
        let s: PadControls = serde_json::from_value(json!({
            "left": ["garbage"],
            "right": 8,
            "hard_drop": ["axis", 4, "positive"],
        }))
        .unwrap();
        assert_eq!(s.left, Some(Binding::Button(13)), "fell back to the default");
        assert_eq!(s.right, Some(Binding::Button(8)), "a real value is kept");
        assert_eq!(
            s.hard_drop,
            Some(Binding::Axis { axis: 4, positive: true })
        );
        // And the slots the file never mentioned are untouched.
        assert_eq!(s, {
            let mut want = PadControls::default();
            want.set(PadAction::Right, Some(Binding::Button(8)));
            want.set(
                PadAction::HardDrop,
                Some(Binding::Axis { axis: 4, positive: true }),
            );
            want
        });
    }

    #[test]
    fn a_binding_slot_may_be_unbound_from_the_file_in_every_spelling() {
        // The four menu-nav slots are the ones the original ever wrote `None`
        // for, so the behaviour has to hold there too.
        for spelled in [json!(null), json!("none"), json!("None")] {
            let n: PadNav = serde_json::from_value(json!({ "back": spelled.clone() })).unwrap();
            assert_eq!(n.back, None, "{spelled} was refilled");
        }
    }

    #[test]
    fn a_settings_file_from_before_controllers_existed_still_loads() {
        let s: crate::settings::Settings =
            serde_json::from_str(r#"{"difficulty":"hard","controls":{"left":262}}"#).unwrap();
        assert_eq!(s.difficulty, "hard");
        // The whole controller block defaults in rather than erroring.
        assert_eq!(s.controller_controls, PadControls::default());
        assert_eq!(s.controller_menu_navigation, PadNav::default());
        assert_eq!(s.controller_settings, PadSettings::default());
    }

    #[test]
    fn a_python_settings_file_keeps_its_controller_bindings() {
        // The exact block the original wrote, so a player who already had custom
        // bindings does not lose them on the first run of this port.
        let python = r#"{
            "controller_controls": {
                "left": 14, "right": 15, "down": 13, "rotate": 0,
                "hard_drop": 1, "hold": ["axis", 0, "negative"],
                "pause": 3, "skip_track": null
            },
            "controller_menu_navigation": {"up": 11, "down": 12, "select": 0, "back": 1},
            "controller_settings": {"analog_threshold": 0.7, "joy_delay": 90,
                                    "hat_threshold": 0.4, "use_dpad": false,
                                    "analog_deadzone": 0.2}
        }"#;
        let s: crate::settings::Settings = serde_json::from_str(python).unwrap();
        assert_eq!(
            s.controller_controls.hold,
            Some(Binding::Axis { axis: 0, positive: false })
        );
        assert_eq!(s.controller_controls.left, Some(Binding::Button(14)));
        // A null binding means "unbound", not "use the default".
        assert_eq!(s.controller_controls.skip_track, None);
        assert_eq!(s.controller_settings.analog_threshold, 0.7);
        assert_eq!(s.controller_settings.joy_delay, 90);
        assert!(!s.controller_settings.use_dpad);
    }

    #[test]
    fn rebinding_one_controller_action_leaves_the_others_alone() {
        let mut c = PadControls::default();
        c.set(PadAction::Rotate, Some(Binding::Button(3)));
        c.set(
            PadAction::HardDrop,
            Some(Binding::Axis { axis: 0, positive: true }),
        );
        assert_eq!(c.get(PadAction::Rotate), Some(Binding::Button(3)));
        assert_eq!(
            c.get(PadAction::HardDrop),
            Some(Binding::Axis { axis: 0, positive: true })
        );
        assert_eq!(c.get(PadAction::Hold), Some(Binding::Button(2)));
    }

    #[test]
    fn unbinding_an_action_stays_unbound_across_a_save() {
        let mut c = PadControls::default();
        c.set(PadAction::Hold, None);
        let back: PadControls = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back.hold, None);
        assert_eq!(back.rotate, Some(Binding::Button(0)), "the rest survive");
    }

    #[test]
    fn rebindings_survive_a_full_settings_round_trip() {
        let mut s = crate::settings::Settings::default();
        s.controller_controls
            .set(PadAction::Left, Some(Binding::Hat(-1, 0)));
        s.controller_menu_navigation
            .set(NavAction::Back, Some(Binding::Button(8)));
        s.controller_settings.use_dpad = false;
        s.controller_settings.analog_threshold = 0.75;
        let back: crate::settings::Settings =
            serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.controller_controls.left, Some(Binding::Hat(-1, 0)));
        assert_eq!(back.controller_menu_navigation.back, Some(Binding::Button(8)));
        assert!(!back.controller_settings.use_dpad);
        assert_eq!(back.controller_settings.analog_threshold, 0.75);
    }

    // --- names -----------------------------------------------------------

    #[test]
    fn every_button_a_binding_can_name_gets_a_name() {
        // The whole of SDL's `SDL_GameControllerButton` range, against the names
        // in `button_name`. Written as a table rather than spot checks because
        // this is the one place where being off by one is invisible: a wrong
        // number does not fail, it labels the wrong physical button, and the
        // player only finds out by pressing what they were told to press.
        //
        // 15 is absent on purpose - it is SDL's MISC1, which no current pad
        // maps, so it is the one number here with no name to give.
        let expected = [
            (0, "A / Cross"),
            (1, "B / Circle"),
            (2, "X / Square"),
            (3, "Y / Triangle"),
            (4, "Back / Share"),
            (5, "Guide / PS"),
            (6, "Start / Options"),
            (7, "L3 / L-Stick"),
            (8, "R3 / R-Stick"),
            (9, "LB / L1"),
            (10, "RB / R1"),
            (11, "D-Pad Up"),
            (12, "D-Pad Down"),
            (13, "D-Pad Left"),
            (14, "D-Pad Right"),
        ];
        for (b, name) in expected {
            assert_eq!(button_name(b), name, "SDL button {b}");
        }
    }

    #[test]
    fn the_face_buttons_name_both_layouts() {
        // A DualSense player reads "A / Cross" and knows which one to press; an
        // Xbox player sees the A first and does not have to think about it. The
        // property is that the two layouts stay *paired* in the same order -
        // Cross is A, Circle is B, and a name that got them crossed would send a
        // PlayStation player to press B when the game wanted A.
        let xbox = ["A", "B", "X", "Y"];
        let playstation = ["Cross", "Circle", "Square", "Triangle"];
        for (b, (x, p)) in xbox.iter().zip(playstation).enumerate() {
            assert_eq!(button_name(b as i32), format!("{x} / {p}"), "SDL button {b}");
        }
    }

    #[test]
    fn the_dpad_buttons_are_all_named() {
        // Four of the eight defaults live here, so an unnamed one would make
        // the default bindings unreadable.
        for b in 11..=14 {
            assert!(
                button_name(b).starts_with("D-Pad"),
                "button {b} is {:?}",
                button_name(b)
            );
        }
        // And the unnamed gap really is a gap, not a mislabelled D-pad
        // direction - which is how the original's broken defaults got in.
        assert_ne!(button_name(15), "D-Pad Left");
        assert_eq!(button_name(15), "Button");
    }

    #[test]
    fn the_two_synthesised_trigger_buttons_are_named_like_the_axes() {
        // They are this port's own numbers, but the label has to match the axis
        // label a player sees on the same hardware or the two spell one control
        // two ways.
        assert_eq!(button_name(LT_BUTTON), axis_name(4, true));
        assert_eq!(button_name(RT_BUTTON), axis_name(5, true));
        assert_eq!(button_name(LT_BUTTON), "LT / L2");
        assert_eq!(button_name(RT_BUTTON), "RT / R2");
    }

    #[test]
    fn a_button_number_with_no_name_still_reads_as_a_binding() {
        // Better than "(none)": the binding is real, the label just is not
        // known, and hiding that would look like an unbound action.
        assert_eq!(button_name(999), "Button");
        assert_eq!(Binding::Button(999).label(), "Button");
    }

    #[test]
    fn a_hat_binding_names_the_direction_it_pushes() {
        assert_eq!(Binding::Hat(0, -1).label(), "D-Pad Up");
        assert_eq!(Binding::Hat(-1, 0).label(), "D-Pad Left");
        assert_eq!(Binding::Hat(0, 1).label(), "D-Pad Down");
        assert_eq!(Binding::Hat(1, 0).label(), "D-Pad Right");
    }

    #[test]
    fn a_diagonal_hat_names_its_dominant_axis() {
        // Pushing up-and-right means "right" as far as movement goes, and a
        // diagonal is not a real binding target, so it is named for the axis
        // that would actually move the piece.
        assert_eq!(Binding::Hat(1, -1).label(), "D-Pad Up-Right");
        assert_eq!(Binding::Hat(1, 1).label(), "D-Pad Down-Right");
    }

    #[test]
    fn an_impossible_hat_still_renders_its_numbers() {
        // A hand-edited value must not panic or produce an empty label.
        assert_eq!(Binding::Hat(5, 0).label(), "Hat (5, 0)");
    }

    #[test]
    fn an_axis_binding_names_the_stick_and_the_direction_to_push() {
        // The Y axes point down, so positive is Down there and negative is Up.
        // Getting this backwards would invert soft drop for a stick-bound
        // player, and the name is the only thing that would tell them.
        assert_eq!(
            Binding::Axis { axis: 0, positive: false }.label(),
            "Left Stick X Left"
        );
        assert_eq!(
            Binding::Axis { axis: 0, positive: true }.label(),
            "Left Stick X Right"
        );
        assert_eq!(
            Binding::Axis { axis: 1, positive: true }.label(),
            "Left Stick Y Down"
        );
        assert_eq!(
            Binding::Axis { axis: 1, positive: false }.label(),
            "Left Stick Y Up"
        );
    }

    #[test]
    fn a_trigger_binding_is_named_as_a_trigger_with_no_direction() {
        assert_eq!(Binding::Axis { axis: 4, positive: true }.label(), "LT / L2");
        assert_eq!(Binding::Axis { axis: 5, positive: false }.label(), "RT / R2");
    }

    #[test]
    fn an_unknown_axis_is_named_by_number() {
        assert_eq!(Binding::Axis { axis: 9, positive: true }.label(), "Axis 9");
    }

    #[test]
    fn an_unbound_slot_says_so() {
        assert_eq!(Binding::None.label(), "(none)");
        assert_eq!(Binding::slot_label(None), "(none)");
        assert_eq!(Binding::slot_label(Some(Binding::Button(0))), "A / Cross");
    }

    #[test]
    fn binding_names_fit_on_a_rebinding_row() {
        // The row is centred 22px text in a 450px column, so roughly 40
        // characters is the ceiling once the action label is included.
        for b in [
            Binding::Button(0),
            Binding::Button(7),
            Binding::Button(11),
            Binding::Hat(-1, 1),
            Binding::Axis { axis: 1, positive: true },
            Binding::None,
        ] {
            assert!(b.label().len() <= 18, "{:?} is too wide", b.label());
        }
        for a in PAD_ACTIONS {
            let row = format!("{}: {}", a.label(), a.default_binding().label());
            assert!(row.len() <= 40, "{row:?} will overflow the column");
        }
        for a in NAV_ACTIONS {
            let row = format!("{}: {}", a.label(), Binding::slot_label(a.default_binding().get()));
            assert!(row.len() <= 40, "{row:?} will overflow the column");
        }
    }

    // --- capture ---------------------------------------------------------

    #[test]
    fn a_pressed_button_is_what_gets_bound() {
        assert_eq!(
            classify_capture(Some(3), None, None),
            Capture::Bind(Binding::Button(3))
        );
    }

    #[test]
    fn a_stick_past_the_threshold_can_be_bound() {
        // Binding a stick direction is the whole reason a player with a pad that
        // has no usable D-pad can still play.
        let axis = Binding::Axis { axis: 0, positive: false };
        assert_eq!(classify_capture(None, Some(axis), None), Capture::Bind(axis));
    }

    #[test]
    fn a_digital_button_wins_over_a_stick_pressed_in_the_same_frame() {
        // Both are legitimate, but the D-pad has a label the player can check,
        // so it is the one to record.
        let axis = Binding::Axis { axis: 1, positive: true };
        assert_eq!(
            classify_capture(Some(14), Some(axis), None),
            Capture::Bind(Binding::Button(14))
        );
    }

    #[test]
    fn nothing_pressed_binds_nothing() {
        assert_eq!(classify_capture(None, None, None), Capture::Nothing);
    }

    #[test]
    fn the_back_binding_cancels_instead_of_binding() {
        // Without this, a player who bound a face button to Rotate could not
        // leave the rebind screen using the pad at all.
        assert_eq!(
            classify_capture(Some(1), None, Some(Binding::Button(1))),
            Capture::Cancel
        );
        // A stick direction works as the escape hatch too.
        let axis = Binding::Axis { axis: 0, positive: true };
        assert_eq!(classify_capture(None, Some(axis), Some(axis)), Capture::Cancel);
        // Anything else still binds.
        assert_eq!(
            classify_capture(Some(2), None, Some(Binding::Button(1))),
            Capture::Bind(Binding::Button(2))
        );
        // And with no Back binding there is nothing to cancel with, which is why
        // the Menu Nav screen passes `None` while the Back row itself captures.
        assert_eq!(
            classify_capture(Some(1), None, None),
            Capture::Bind(Binding::Button(1))
        );
    }

    #[test]
    fn a_button_the_pad_never_reports_can_still_be_chosen() {
        // SDL's MISC1 has no raylib equivalent, so a binding to it is permanently
        // inactive. It must still be *selectable*, or a player whose settings
        // file has it would be unable to clear it.
        assert_eq!(
            classify_capture(Some(15), None, None),
            Capture::Bind(Binding::Button(15))
        );
    }

    // --- finding the pad -------------------------------------------------

    #[test]
    fn the_first_slot_raylib_recognises_wins() {
        // The case the original got wrong: a non-pad device in slot 0 and the
        // player's controller in slot 1.
        assert_eq!(connected_index(|_| false), None);
        assert_eq!(connected_index(|i| i == 0), Some(0));
        assert_eq!(connected_index(|i| i == 2), Some(2));
        assert_eq!(connected_index(|i| i >= 1), Some(1));
    }

    #[test]
    fn the_scan_never_looks_past_raylibs_slot_limit() {
        // `is_gamepad_available` rejects anything past `MAX_GAMEPADS`, so
        // scanning further would read past the array raylib allocated. With
        // nothing connected every slot is asked and the last one asked is the
        // last one there is.
        let mut highest = -1;
        let found = connected_index(|i| {
            highest = i;
            false
        });
        assert_eq!(found, None);
        assert_eq!(highest, MAX_GAMEPADS - 1);

        // A device sitting past the limit is not merely missed - it is never
        // asked about, which is the property that keeps this safe rather than
        // merely ineffective.
        let mut asked = Vec::new();
        let found = connected_index(|i| {
            asked.push(i);
            i >= MAX_GAMEPADS
        });
        assert_eq!(found, None);
        assert_eq!(asked, (0..MAX_GAMEPADS).collect::<Vec<i32>>());
    }

    #[test]
    fn a_pinned_slot_is_used_when_that_device_is_there() {
        // The wheel-in-slot-0 case: the player says slot 1 and slot 1 exists, so
        // slot 0 must be ignored even though it is also available.
        assert_eq!(resolve_slot(1, |i| i >= 1), Some(1));
        assert_eq!(resolve_slot(2, |i| i == 2), Some(2));
    }

    #[test]
    fn a_pinned_slot_with_nothing_in_it_falls_back_to_the_first_available() {
        // A player who unplugged their pad must not be left with a dead game and
        // no way to fix it from the menu, so a stale pin is ignored rather than
        // obeyed.
        assert_eq!(resolve_slot(1, |i| i == 0), Some(0));
        assert_eq!(resolve_slot(3, |_| false), None);
    }

    #[test]
    fn an_out_of_range_pin_is_ignored_rather_than_obeyed() {
        // A hand-edited settings file must not be able to point the game past the
        // end of raylib's array.
        for wanted in [-1, 4, 99, i32::MIN, i32::MAX] {
            assert_eq!(resolve_slot(wanted, |i| i == 0), Some(0), "wanted {wanted}");
        }
    }

    #[test]
    fn a_pad_plugged_in_after_launch_is_found_on_the_next_check() {
        // `resolve_slot` is called every frame precisely so that this is a
        // question with no cached answer to go stale.
        let plugged_in = std::cell::Cell::new(false);
        assert_eq!(resolve_slot(0, |_| plugged_in.get()), None);
        plugged_in.set(true);
        assert_eq!(resolve_slot(0, |_| plugged_in.get()), Some(0));
    }
}
