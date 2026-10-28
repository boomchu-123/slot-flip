#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Btn {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    L1,
    R1,
    L2,
    R2,
    L3,
    R3,
    Start,
    Select,
    Menu,
    VolUp,
    VolDown,
    Power,
    Lid,
}

impl Btn {
    /// Stable lowercase name used in config files (e.g. Slot/Config/buttons.ini).
    pub fn as_name(self) -> &'static str {
        match self {
            Btn::Up => "up",
            Btn::Down => "down",
            Btn::Left => "left",
            Btn::Right => "right",
            Btn::A => "a",
            Btn::B => "b",
            Btn::X => "x",
            Btn::Y => "y",
            Btn::L1 => "l1",
            Btn::R1 => "r1",
            Btn::L2 => "l2",
            Btn::R2 => "r2",
            Btn::L3 => "l3",
            Btn::R3 => "r3",
            Btn::Start => "start",
            Btn::Select => "select",
            Btn::Menu => "menu",
            Btn::VolUp => "volup",
            Btn::VolDown => "voldown",
            Btn::Power => "power",
            Btn::Lid => "lid",
        }
    }

    /// Parse a button name from a config file. Case-insensitive; a few common
    /// aliases are accepted so hand-edited files are forgiving.
    pub fn from_name(name: &str) -> Option<Btn> {
        Some(match name.trim().to_ascii_lowercase().as_str() {
            "up" => Btn::Up,
            "down" => Btn::Down,
            "left" => Btn::Left,
            "right" => Btn::Right,
            "a" => Btn::A,
            "b" => Btn::B,
            "x" => Btn::X,
            "y" => Btn::Y,
            "l1" | "l" | "lb" => Btn::L1,
            "r1" | "r" | "rb" => Btn::R1,
            "l2" | "lt" => Btn::L2,
            "r2" | "rt" => Btn::R2,
            "l3" => Btn::L3,
            "r3" => Btn::R3,
            "start" => Btn::Start,
            "select" => Btn::Select,
            "menu" | "guide" | "home" => Btn::Menu,
            "volup" | "vol_up" => Btn::VolUp,
            "voldown" | "vol_down" => Btn::VolDown,
            "power" => Btn::Power,
            "lid" => Btn::Lid,
            _ => return None,
        })
    }

    /// Every button, for enumerating bindings in a menu.
    pub const ALL: [Btn; 21] = [
        Btn::Up,
        Btn::Down,
        Btn::Left,
        Btn::Right,
        Btn::A,
        Btn::B,
        Btn::X,
        Btn::Y,
        Btn::L1,
        Btn::R1,
        Btn::L2,
        Btn::R2,
        Btn::L3,
        Btn::R3,
        Btn::Start,
        Btn::Select,
        Btn::Menu,
        Btn::VolUp,
        Btn::VolDown,
        Btn::Power,
        Btn::Lid,
    ];
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RawEvent {
    Down(Btn),
    Up(Btn),
}

pub type Millis = u64;

pub trait InputSource {
    fn poll(&mut self, now: Millis) -> Vec<RawEvent>;
}
