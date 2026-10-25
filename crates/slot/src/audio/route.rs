//! The Flip's codec has a routing switch that is not the volume: the output
//! path (speaker, headphones, off) is a separate ALSA control.
//!
//! On the Flip it used to be set by whichever launcher was running (MinUI's
//! `keymon`, or the firmware's own), which is why slot could play sound while
//! doing nothing about it. Booting straight into slot leaves no launcher daemon
//! to do that, and with the path left at `OFF` ALSA accepts every sample, the
//! log looks perfect, and the game is silent. So slot sets it itself, and
//! follows the headphone jack, which it already polls for the volume icon.
//!
//! Setting it when it is already right is harmless, and it is only written when
//! the jack actually changes, so this does not fight anything else that cares.

#[cfg(feature = "flip")]
use std::process::{Command, Stdio};

/// The path name this device uses for the speaker or the headphones. Kept apart
/// from the call so it can be asserted without a sound card.
pub fn path_for(headphones: bool) -> &'static str {
    if headphones {
        "HP"
    } else {
        "SPK"
    }
}

/// Point the codec at the speaker or the headphones. Errors are ignored on
/// purpose: a device with no such control simply keeps doing whatever it did.
pub fn apply(headphones: bool) {
    #[cfg(feature = "flip")]
    {
        let _ = Command::new("amixer")
            .args(["-q", "cset", "name=Playback Path", path_for(headphones)])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(feature = "flip"))]
    let _ = headphones;
}

#[cfg(test)]
mod tests {
    use super::path_for;

    #[test]
    fn the_speaker_and_the_headphones_are_different_paths() {
        assert_eq!(path_for(false), "SPK");
        assert_eq!(path_for(true), "HP");
        assert_ne!(path_for(true), path_for(false));
    }
}
