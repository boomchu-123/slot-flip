//! The Mini Plus' sound: the vendor output first, ALSA if that cannot start.
//!
//! ALSA is not expected to work on this console (its card is busy with no
//! process holding it), but it is kept as the fallback so a unit whose vendor
//! library is somewhere unexpected behaves as it did before this backend, and
//! so `SLOT_AUDIO=alsa` can force it when comparing the two.

#![cfg_attr(not(feature = "miyoomini"), allow(dead_code))]

use std::sync::Arc;

use super::ring::Ring;
use super::sink::{AudioError, AudioSink};
use super::{AlsaSink, MiAoSink};

pub struct MiyooAudio {
    vendor: MiAoSink,
    alsa: AlsaSink,
    use_alsa: bool,
}

impl MiyooAudio {
    pub fn new() -> Self {
        MiyooAudio {
            vendor: MiAoSink::new(),
            alsa: AlsaSink::new(),
            use_alsa: false,
        }
    }
}

impl Default for MiyooAudio {
    fn default() -> Self {
        MiyooAudio::new()
    }
}

/// Which output to try first: the vendor's unless told otherwise.
fn forced_alsa(choice: Option<&str>) -> bool {
    choice.is_some_and(|c| c.eq_ignore_ascii_case("alsa"))
}

impl AudioSink for MiyooAudio {
    fn open(&mut self, sample_rate: u32) -> Result<(), AudioError> {
        self.use_alsa = forced_alsa(std::env::var("SLOT_AUDIO").ok().as_deref());
        if !self.use_alsa {
            match self.vendor.open(sample_rate) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    eprintln!("slot: audio: the vendor output could not start ({e}); trying ALSA");
                    self.use_alsa = true;
                }
            }
        } else {
            eprintln!("slot: audio: SLOT_AUDIO=alsa, using ALSA");
        }
        self.alsa.open(sample_rate)
    }

    fn ring(&self) -> Arc<Ring> {
        match self.use_alsa {
            true => self.alsa.ring(),
            false => self.vendor.ring(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::forced_alsa;

    #[test]
    fn alsa_is_only_used_first_when_asked_for() {
        assert!(!forced_alsa(None));
        assert!(!forced_alsa(Some("")));
        assert!(!forced_alsa(Some("vendor")));
        assert!(forced_alsa(Some("alsa")));
        assert!(forced_alsa(Some("ALSA")));
    }
}
