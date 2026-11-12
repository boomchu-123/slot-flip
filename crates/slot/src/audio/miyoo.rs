//! The Mini Plus' sound: through the console's audio server the way its own
//! launcher plays, then the vendor calls, then ALSA.
//!
//! The launcher's sound goes through `/dev/dsp`, which the preloaded `libpadsp`
//! answers by handing the audio to the server that owns the hardware
//! (`OssSink`). That is tried first. The vendor library (`MiAoSink`) comes next:
//! a tester's log showed it refused (`MI_AO_SetPubAttr` answers `0xa0052009`)
//! for as long as the server runs, and the preload does not stand in front of
//! those calls, so it is only the fallback. Taking the device from the server
//! (`Config/audio` holding `takeover`) is kept for comparison: it was refused on
//! a tester's console even with the server stopped, and it costs the console its
//! own sounds until slot exits. `alsa` selects ALSA, which is not expected to
//! work on this console.

#![cfg_attr(not(feature = "miyoomini"), allow(dead_code))]

use std::sync::Arc;

use super::ring::Ring;
use super::sink::{AudioError, AudioSink};
use super::{AlsaSink, MiAoSink, OssSink};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Output {
    /// `/dev/dsp`, answered by the audio server through the preloaded shim.
    Oss,
    /// The vendor calls, answered by whatever sits in front of them.
    Vendor,
    /// The vendor calls, after stopping the audio server to take the device.
    Takeover,
    Alsa,
}

pub struct MiyooAudio {
    oss: OssSink,
    vendor: MiAoSink,
    takeover: MiAoSink,
    alsa: AlsaSink,
    using: Output,
}

impl MiyooAudio {
    pub fn new() -> Self {
        MiyooAudio {
            oss: OssSink::new(),
            vendor: MiAoSink::new(),
            takeover: MiAoSink::taking(),
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
        Some("takeover") => Output::Takeover,
        Some("vendor" | "miao" | "mi_ao") => Output::Vendor,
        _ => Output::Oss,
    }
}

/// The outputs to try, in order, once the first is chosen. The audio server's
/// output falls back to the vendor calls and then ALSA; the others fall back to
/// ALSA only, so that a choice made to compare them is not hidden by another.
fn order(first: Output) -> Vec<Output> {
    match first {
        Output::Oss => vec![Output::Oss, Output::Vendor, Output::Alsa],
        Output::Alsa => vec![Output::Alsa],
        other => vec![other, Output::Alsa],
    }
}

impl AudioSink for MiyooAudio {
    fn open(&mut self, sample_rate: u32) -> Result<(), AudioError> {
        let first = chosen(std::env::var("SLOT_AUDIO").ok().as_deref());
        match first {
            Output::Takeover => eprintln!(
                "slot: audio: SLOT_AUDIO=takeover, the audio server will be stopped if the device is refused"
            ),
            Output::Vendor => eprintln!("slot: audio: SLOT_AUDIO=vendor, the vendor calls first"),
            Output::Alsa => eprintln!("slot: audio: SLOT_AUDIO=alsa, using ALSA"),
            Output::Oss => {}
        }
        let mut last = AudioError::Device("no output".into());
        for next in order(first) {
            self.using = next;
            let tried = match next {
                Output::Oss => self
                    .oss
                    .open(sample_rate)
                    .map_err(|e| ("the audio server's output", e)),
                Output::Vendor => self
                    .vendor
                    .open(sample_rate)
                    .map_err(|e| ("the vendor output", e)),
                Output::Takeover => self
                    .takeover
                    .open(sample_rate)
                    .map_err(|e| ("the vendor output", e)),
                Output::Alsa => self.alsa.open(sample_rate).map_err(|e| ("ALSA", e)),
            };
            match tried {
                Ok(()) => return Ok(()),
                Err((name, e)) => {
                    eprintln!("slot: audio: {name} could not start ({e})");
                    last = e;
                }
            }
        }
        Err(last)
    }

    fn ring(&self) -> Arc<Ring> {
        match self.using {
            Output::Oss => self.oss.ring(),
            Output::Vendor => self.vendor.ring(),
            Output::Takeover => self.takeover.ring(),
            Output::Alsa => self.alsa.ring(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{chosen, order, Output};

    #[test]
    fn the_audio_servers_output_is_first_unless_told_otherwise() {
        assert_eq!(chosen(None), Output::Oss);
        assert_eq!(chosen(Some("")), Output::Oss);
        assert_eq!(chosen(Some("oss")), Output::Oss);
        assert_eq!(chosen(Some("nonsense")), Output::Oss);
    }

    #[test]
    fn the_other_outputs_are_only_first_when_asked_for() {
        assert_eq!(chosen(Some("alsa")), Output::Alsa);
        assert_eq!(chosen(Some("ALSA")), Output::Alsa);
        assert_eq!(chosen(Some("takeover")), Output::Takeover);
        assert_eq!(chosen(Some("Takeover")), Output::Takeover);
        assert_eq!(chosen(Some("vendor")), Output::Vendor);
        assert_eq!(chosen(Some("mi_ao")), Output::Vendor);
    }

    #[test]
    fn the_default_falls_back_through_the_vendor_calls_to_alsa() {
        assert_eq!(
            order(Output::Oss),
            [Output::Oss, Output::Vendor, Output::Alsa]
        );
    }

    #[test]
    fn a_comparison_falls_back_to_alsa_only() {
        assert_eq!(order(Output::Takeover), [Output::Takeover, Output::Alsa]);
        assert_eq!(order(Output::Vendor), [Output::Vendor, Output::Alsa]);
        assert_eq!(order(Output::Alsa), [Output::Alsa]);
    }
}
