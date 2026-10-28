//! The Mini Plus' sound: the vendor interface the way the launcher's own sound
//! reaches it, with ALSA as the fallback.
//!
//! The launcher's SDL calls the vendor library (`libmi_ao`) like any program
//! that links it, and the console's preloaded `libpadsp` sits in front of those
//! calls and hands them to the audio server that owns the hardware. So the
//! vendor backend (`MiAoSink`) looks its calls up the same way and leaves the
//! server alone. Taking the device from the server instead (`Config/audio`
//! holding `takeover`) is kept for comparison: on a tester's console it was
//! refused even with the server stopped, and it costs the console its own
//! sounds until slot exits. `alsa` selects ALSA, which is not expected to work
//! on this console.

#![cfg_attr(not(feature = "miyoomini"), allow(dead_code))]

use std::sync::Arc;

use super::ring::Ring;
use super::sink::{AudioError, AudioSink};
use super::{AlsaSink, MiAoSink};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Output {
    /// The vendor calls, answered by whatever sits in front of them.
    Vendor,
    /// The vendor calls, after stopping the audio server to take the device.
    Takeover,
    Alsa,
}

pub struct MiyooAudio {
    vendor: MiAoSink,
    takeover: MiAoSink,
    alsa: AlsaSink,
    using: Output,
}

impl MiyooAudio {
    pub fn new() -> Self {
        MiyooAudio {
            vendor: MiAoSink::new(),
            takeover: MiAoSink::taking(),
            alsa: AlsaSink::new(),
            using: Output::Vendor,
        }
    }
}

impl Default for MiyooAudio {
    fn default() -> Self {
        MiyooAudio::new()
    }
}

/// Which output to try first: the vendor calls, unless told otherwise.
fn chosen(choice: Option<&str>) -> Output {
    match choice.map(str::to_ascii_lowercase).as_deref() {
        Some("alsa") => Output::Alsa,
        Some("takeover") => Output::Takeover,
        _ => Output::Vendor,
    }
}

impl AudioSink for MiyooAudio {
    fn open(&mut self, sample_rate: u32) -> Result<(), AudioError> {
        let first = chosen(std::env::var("SLOT_AUDIO").ok().as_deref());
        self.using = first;
        let tried = match first {
            Output::Vendor => Some((&mut self.vendor, "the vendor output")),
            Output::Takeover => {
                eprintln!("slot: audio: SLOT_AUDIO=takeover, the audio server will be stopped if the device is refused");
                Some((&mut self.takeover, "the vendor output"))
            }
            Output::Alsa => {
                eprintln!("slot: audio: SLOT_AUDIO=alsa, using ALSA");
                None
            }
        };
        if let Some((sink, name)) = tried {
            match sink.open(sample_rate) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    eprintln!("slot: audio: {name} could not start ({e}); trying ALSA");
                    self.using = Output::Alsa;
                }
            }
        }
        self.alsa.open(sample_rate)
    }

    fn ring(&self) -> Arc<Ring> {
        match self.using {
            Output::Vendor => self.vendor.ring(),
            Output::Takeover => self.takeover.ring(),
            Output::Alsa => self.alsa.ring(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{chosen, Output};

    #[test]
    fn the_vendor_calls_are_first_unless_told_otherwise() {
        assert_eq!(chosen(None), Output::Vendor);
        assert_eq!(chosen(Some("")), Output::Vendor);
        assert_eq!(chosen(Some("vendor")), Output::Vendor);
        assert_eq!(chosen(Some("nonsense")), Output::Vendor);
    }

    #[test]
    fn alsa_and_the_takeover_are_only_first_when_asked_for() {
        assert_eq!(chosen(Some("alsa")), Output::Alsa);
        assert_eq!(chosen(Some("ALSA")), Output::Alsa);
        assert_eq!(chosen(Some("takeover")), Output::Takeover);
        assert_eq!(chosen(Some("Takeover")), Output::Takeover);
    }
}
