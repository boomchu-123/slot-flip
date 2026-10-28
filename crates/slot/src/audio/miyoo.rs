//! The Mini Plus' sound: through the console's audio server the way its own
//! launcher plays, with ALSA as the fallback.
//!
//! The vendor library (`MiAoSink`) is still here, but only when asked for. It
//! sets the hardware up itself, which means taking it from the firmware's audio
//! server, and on a tester's console that was refused even with the server
//! stopped while costing the console its own sounds. `Config/audio` (or
//! `SLOT_AUDIO`) can still select it, or ALSA, for comparing the three.

#![cfg_attr(not(feature = "miyoomini"), allow(dead_code))]

use std::sync::Arc;

use super::ring::Ring;
use super::sink::{AudioError, AudioSink};
use super::{AlsaSink, MiAoSink, OssSink};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Output {
    /// `/dev/dsp`, answered by the audio server through the preloaded shim.
    Oss,
    /// The vendor library, taking the device from the audio server.
    Vendor,
    Alsa,
}

pub struct MiyooAudio {
    oss: OssSink,
    vendor: MiAoSink,
    alsa: AlsaSink,
    using: Output,
}

impl MiyooAudio {
    pub fn new() -> Self {
        MiyooAudio {
            oss: OssSink::new(),
            vendor: MiAoSink::new(),
            alsa: AlsaSink::new(),
            using: Output::Oss,
        }
    }
}

impl Default for MiyooAudio {
    fn default() -> Self {
        MiyooAudio::new()
    }
}

/// Which output to try first: the audio server's unless told otherwise.
fn chosen(choice: Option<&str>) -> Output {
    match choice.map(str::to_ascii_lowercase).as_deref() {
        Some("alsa") => Output::Alsa,
        Some("vendor" | "miao" | "mi_ao") => Output::Vendor,
        _ => Output::Oss,
    }
}

impl AudioSink for MiyooAudio {
    fn open(&mut self, sample_rate: u32) -> Result<(), AudioError> {
        let first = chosen(std::env::var("SLOT_AUDIO").ok().as_deref());
        self.using = first;
        match first {
            Output::Oss => {
                match self.oss.open(sample_rate) {
                    Ok(()) => return Ok(()),
                    Err(e) => {
                        eprintln!("slot: audio: the audio server's output could not start ({e}); trying ALSA");
                        self.using = Output::Alsa;
                    }
                }
            }
            Output::Vendor => {
                eprintln!(
                    "slot: audio: SLOT_AUDIO=vendor, taking the device from the audio server"
                );
                match self.vendor.open(sample_rate) {
                    Ok(()) => return Ok(()),
                    Err(e) => {
                        eprintln!(
                            "slot: audio: the vendor output could not start ({e}); trying ALSA"
                        );
                        self.using = Output::Alsa;
                    }
                }
            }
            Output::Alsa => eprintln!("slot: audio: SLOT_AUDIO=alsa, using ALSA"),
        }
        self.alsa.open(sample_rate)
    }

    fn ring(&self) -> Arc<Ring> {
        match self.using {
            Output::Oss => self.oss.ring(),
            Output::Vendor => self.vendor.ring(),
            Output::Alsa => self.alsa.ring(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{chosen, Output};

    #[test]
    fn the_audio_servers_output_is_first_unless_told_otherwise() {
        assert_eq!(chosen(None), Output::Oss);
        assert_eq!(chosen(Some("")), Output::Oss);
        assert_eq!(chosen(Some("oss")), Output::Oss);
        assert_eq!(chosen(Some("nonsense")), Output::Oss);
    }

    #[test]
    fn alsa_and_the_vendor_output_are_only_first_when_asked_for() {
        assert_eq!(chosen(Some("alsa")), Output::Alsa);
        assert_eq!(chosen(Some("ALSA")), Output::Alsa);
        assert_eq!(chosen(Some("vendor")), Output::Vendor);
        assert_eq!(chosen(Some("Vendor")), Output::Vendor);
        assert_eq!(chosen(Some("mi_ao")), Output::Vendor);
    }
}
