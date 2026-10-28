use slot_input::{Action, Btn};
use slot_retro::ButtonMask;
use slot_store::{Controls, Device, GameButton, Modifier, Platform};

use super::bind::device_btn;

#[derive(Default)]
pub struct Pad {
    held: Vec<Btn>,
    chorded: Vec<Btn>,
}

impl Pad {
    pub fn apply(&mut self, action: Action) {
        let (list, b, down) = match action {
            Action::GbaDown(b) => (&mut self.held, b, true),
            Action::GbaUp(b) => (&mut self.held, b, false),
            Action::MenuChordDown(b) => (&mut self.chorded, b, true),
            Action::MenuChordUp(b) => (&mut self.chorded, b, false),
            _ => return,
        };
        list.retain(|x| *x != b);
        if down {
            list.push(b);
        }
    }

    pub fn mask(&self, controls: &Controls) -> ButtonMask {
        self.mask_for(controls, Platform::Gba)
    }

    /// What the core sees for `platform`. The Game Boy family is the 12 named
    /// buttons; a PlayStation pad keeps the shared ones and adds its own, which the
    /// Flip's remaining buttons stand in for (see `PS1_EXTRAS`).
    pub fn mask_for(&self, controls: &Controls, platform: Platform) -> ButtonMask {
        let disc = platform.native_pad();
        let bits = GameButton::ALL
            .into_iter()
            // Turbo is a GBA feature; on a disc its buttons are Triangle and Square.
            .filter(|g| !(disc && matches!(g, GameButton::TurboA | GameButton::TurboB)))
            .filter(|g| {
                controls.game(*g).is_some_and(|binding| {
                    let b = device_btn(binding.button);
                    match binding.modifier {
                        None => self.held.contains(&b),
                        Some(Modifier::Menu) => self.chorded.contains(&b),
                        Some(Modifier::Select | Modifier::Button(_)) => false,
                    }
                })
            })
            .fold(0, |bits, g| bits | bit(g));
        let extras = extras(platform)
            .iter()
            .filter(|(device, _)| self.held.contains(&device_btn(*device)))
            .fold(0, |bits, (_, bit)| bits | bit);
        ButtonMask(bits | extras | self.clicks(platform))
    }

    /// The two stick clicks. They are fixed rather than rebindable: only the
    /// gamepad node reports them, and only a PlayStation pad has them.
    fn clicks(&self, platform: Platform) -> u16 {
        if platform != Platform::Ps1 {
            return 0;
        }
        let bit = |btn, mask| if self.held.contains(&btn) { mask } else { 0 };
        bit(Btn::L3, ButtonMask::L3) | bit(Btn::R3, ButtonMask::R3)
    }

    pub fn clear(&mut self) {
        self.held.clear();
        self.chorded.clear();
    }
}

/// The PlayStation buttons with no GBA equivalent, on the Flip's physical buttons by
/// position: top (X) is Triangle, left (Y) is Square, and the lower shoulders are
/// L2 and R2. Cross is the bottom button (B) and Circle the right (A), which the
/// shared A/B bindings already give. L3 and R3 are the two sticks pushed in, which
/// only the gamepad node reports (see `device::read_node`).
const PS1_EXTRAS: [(Device, u16); 4] = [
    (Device::X, ButtonMask::X),
    (Device::Y, ButtonMask::Y),
    (Device::L2, ButtonMask::L2),
    (Device::R2, ButtonMask::R2),
];

/// The buttons a native pad adds to the shared ones, and which physical key stands in
/// for each.
pub(crate) fn extras(platform: Platform) -> &'static [(Device, u16)] {
    match platform {
        Platform::Ps1 => &PS1_EXTRAS,
        Platform::Snes | Platform::Md => &SNES_EXTRAS,
        _ => &[],
    }
}

/// A Super Nintendo pad has X and Y where a Game Boy has none: the top and left face
/// buttons. L and R are the shoulders the GBA already has, so they need nothing here.
/// A Mega Drive's six-button pad is read the same way: the core takes the left face
/// button as A, B and the right one as C, and X (top), L and R as Y, X and Z.
const SNES_EXTRAS: [(Device, u16); 2] = [(Device::X, ButtonMask::X), (Device::Y, ButtonMask::Y)];

fn bit(g: GameButton) -> u16 {
    match g {
        GameButton::Up => ButtonMask::UP,
        GameButton::Down => ButtonMask::DOWN,
        GameButton::Left => ButtonMask::LEFT,
        GameButton::Right => ButtonMask::RIGHT,
        GameButton::A => ButtonMask::A,
        GameButton::B => ButtonMask::B,
        GameButton::Select => ButtonMask::SELECT,
        GameButton::Start => ButtonMask::START,
        GameButton::L => ButtonMask::L,
        GameButton::R => ButtonMask::R,
        GameButton::TurboA => ButtonMask::X,
        GameButton::TurboB => ButtonMask::Y,
    }
}
