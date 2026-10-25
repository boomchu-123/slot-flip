macro_rules! eprintln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

mod core;
mod ffi;
mod libretro;
mod link;
mod mock;
mod rumble;

pub use core::{AvInfo, ButtonMask, CoreError, RetroCore, GBA_H, GBA_W};
pub use ffi::{
    NETPACKET_BROADCAST, NETPACKET_FLUSH_HINT, NETPACKET_RELIABLE, NETPACKET_UNRELIABLE,
    NETPACKET_UNSEQUENCED,
};
pub use libretro::{
    analog_stats, LibretroCore, SystemInformation, SWAP_CLOSE_FRAME, SWAP_SWITCH_FRAME,
};
pub use link::{Link, LinkChannel, LoopbackLink};
pub use mock::MockCore;
pub use rumble::Rumble;
