/// The upstream slot release this fork is based on. Set it in the same commit
/// that merges that upstream tag (AGENTS section 5), because it is the only
/// place an owner can see it: the About sticker prints it next to our version.
pub const UPSTREAM: &str = "1.9.0";

/// The device this binary was compiled for, printed at startup.
pub const DEVICE: &str = if cfg!(feature = "host") {
    "host"
} else if cfg!(feature = "miyoomini") {
    "miyoomini"
} else if cfg!(feature = "flip") {
    "my355"
} else {
    "device"
};

#[derive(Copy, Clone, Debug)]
pub struct Build {
    pub version: &'static str,
    pub hash: &'static str,
    pub date: &'static str,
}

impl Build {
    pub fn current() -> Build {
        Build {
            version: env!("SLOT_VERSION"),
            hash: env!("SLOT_GIT_HASH"),
            date: env!("SLOT_BUILD_DATE"),
        }
    }

    pub fn serial(&self) -> String {
        self.hash.to_uppercase()
    }
}
