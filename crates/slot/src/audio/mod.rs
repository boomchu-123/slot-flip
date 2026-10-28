mod alsa;
#[cfg(feature = "host")]
mod host;
#[cfg(not(feature = "host"))]
mod miao;
#[cfg(not(feature = "host"))]
mod miyoo;
mod pacer;
mod ring;
pub mod route;
mod sfx;
mod silence;
mod sink;
mod stub;
pub mod volume;

pub use alsa::AlsaSink;
#[cfg(feature = "host")]
pub use host::HostAudio;
#[cfg(not(feature = "host"))]
pub use miao::MiAoSink;
#[cfg(feature = "miyoomini")]
pub use miyoo::MiyooAudio;
pub use pacer::Pacer;
pub use ring::{ring_capacity, Ring};
pub use sfx::Sfx;
pub use silence::Silence;
pub use sink::{AudioError, AudioSink};
pub use stub::StubSink;

pub const OUTPUT_HZ: u32 = 48_000;

pub fn opened_sink() -> Box<dyn AudioSink> {
    let mut sink = open_sink();
    if let Err(e) = sink.open(OUTPUT_HZ) {
        eprintln!("slot: audio: {e}");
        // Say who owns the sound devices now rather than only when a game
        // starts making noise; on a handheld that is the boot that matters.
        #[cfg(not(feature = "host"))]
        alsa::log_holders();
    }
    sink
}

#[cfg(feature = "host")]
pub fn open_sink() -> Box<dyn AudioSink> {
    if std::env::var("SLOT_SILENT").as_deref() == Ok("1") {
        return Box::new(StubSink::draining());
    }
    Box::new(HostAudio::new())
}

#[cfg(not(feature = "host"))]
pub fn open_sink() -> Box<dyn AudioSink> {
    // SLOT_SILENT swaps in a stub that drains the ring at a steady simulated
    // rate. Handy on the Flip to tell an ALSA pacing problem from everything
    // else: if games run smoothly silent, the audio device is the culprit.
    if std::env::var_os("SLOT_SILENT").is_some() {
        return Box::new(StubSink::draining());
    }
    // The Mini Plus' sound card cannot be opened through ALSA, so it goes
    // through the vendor output and falls back to ALSA only if that cannot start.
    #[cfg(feature = "miyoomini")]
    return Box::new(MiyooAudio::new());
    #[cfg(not(feature = "miyoomini"))]
    Box::new(AlsaSink::new())
}
