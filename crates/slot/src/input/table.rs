use slot_input::{anywhere, Action, Contexts, Entry};
use slot_store::{
    Binding, CarouselAction, CarouselKeys, Controls, Device, Modifier, Platform, Shortcut,
    ShortcutKeys,
};
use slot_ui::QuickRow;

use super::bind::{chord, shortcut_action};

#[derive(Copy, Clone, Debug)]
struct Row {
    row: QuickRow,
    binding: Binding,
    action: Option<Action>,
    contexts: Contexts,
}

pub struct Table {
    rows: Vec<Row>,
}

impl Table {
    pub fn new(
        controls: &Controls,
        shortcuts: &ShortcutKeys,
        carousel: &CarouselKeys,
        turbo: bool,
    ) -> Table {
        Table::for_platform(controls, shortcuts, carousel, turbo, Platform::Gba)
    }

    /// The table for the system being played. A disc's pad uses X, Y, L2 and R2 as
    /// game buttons, so no shortcut may also claim them plain (see `on_native`).
    pub fn for_platform(
        controls: &Controls,
        shortcuts: &ShortcutKeys,
        carousel: &CarouselKeys,
        turbo: bool,
        platform: Platform,
    ) -> Table {
        let disc = platform.native_pad();
        let rows = QuickRow::ALL
            .into_iter()
            .filter_map(|row| {
                if let Some(g) = row.game_button() {
                    let turbo_row = matches!(row, QuickRow::GameTurboA | QuickRow::GameTurboB);
                    if turbo_row && (!turbo || disc) {
                        return None;
                    }
                    return controls.game(g).map(|binding| Row {
                        row,
                        binding,
                        action: None,
                        contexts: Contexts::GAME,
                    });
                }
                if let Some(s) = row.shortcut() {
                    let action = shortcut_action(s);
                    let bound = match disc {
                        true => shortcuts.get(s).and_then(|b| on_native(s, b, platform)),
                        false => shortcuts.get(s),
                    };
                    return bound.map(|binding| Row {
                        row,
                        binding,
                        action: Some(action),
                        contexts: match (anywhere(action), binding.modifier) {
                            (false, _) => Contexts::GAME,
                            (true, None) => Contexts::GAME.with(Contexts::SHELF),
                            (true, Some(_)) => Contexts::ALL,
                        },
                    });
                }
                let a = row.carousel_action()?;
                carousel.get(a).map(|binding| Row {
                    row,
                    binding,
                    action: Some(shelf_action(a)),
                    contexts: Contexts::SHELF,
                })
            })
            .collect();
        Table { rows }
    }

    pub fn entries(&self) -> Vec<Entry> {
        self.rows
            .iter()
            .filter_map(|r| {
                r.action.map(|action| Entry {
                    chord: chord(r.binding),
                    action,
                    contexts: r.contexts,
                })
            })
            .collect()
    }

    pub fn clash(&self, row: QuickRow) -> Option<QuickRow> {
        let mine = self.rows.iter().find(|r| r.row == row)?;
        let console = |r: &Row| r.action.is_none();
        self.rows
            .iter()
            .filter(|r| r.row != row && r.contexts.meets(mine.contexts))
            .find(|r| {
                r.binding == mine.binding
                    || (!console(mine)
                        && !console(r)
                        && (overlaps(mine.binding, r.binding) || overlaps(r.binding, mine.binding)))
            })
            .map(|r| r.row)
    }
}

/// What a shortcut becomes while a system with a native pad is playing. A pad's own
/// buttons are the game's: on a PlayStation that is X, Y, L2 and R2, on a Super
/// Nintendo X and Y. Plain L2 and R2 are rewind and fast-forward everywhere else, so
/// where they are game buttons fast-forward moves to SELECT+R2 and rewind is off. The
/// palette shortcuts only colour a Game Boy, so they give up SELECT+R2 for it. A
/// binding the user chose is only moved if it would take a pad button.
fn on_native(shortcut: Shortcut, binding: Binding, platform: Platform) -> Option<Binding> {
    let pad: Vec<Device> = crate::input::mask::extras(platform)
        .iter()
        .map(|(device, _)| *device)
        .collect();
    match shortcut {
        Shortcut::FastForward
            if pad.contains(&Device::R2) && Some(binding) == Binding::plain(Device::R2) =>
        {
            Binding::with(Modifier::Select, Device::R2)
        }
        Shortcut::Rewind if pad.contains(&Device::L2) => None,
        Shortcut::PaletteBack | Shortcut::PaletteNext => None,
        _ if binding.modifier.is_none() && pad.contains(&binding.button) => None,
        _ => Some(binding),
    }
}

fn shelf_action(a: CarouselAction) -> Action {
    match a {
        CarouselAction::PreviousShelf => Action::PreviousShelf,
        CarouselAction::NextShelf => Action::NextShelf,
        CarouselAction::Favorite => Action::Favorite,
    }
}

fn overlaps(pair: Binding, single: Binding) -> bool {
    let Some(Modifier::Button(first)) = pair.modifier else {
        return false;
    };
    single.modifier.is_none()
        && (single.button == first || (!first.leads() && single.button == pair.button))
}
