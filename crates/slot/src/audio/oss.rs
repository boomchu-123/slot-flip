//! Sound on the Miyoo Mini Plus through `/dev/dsp`, the way MinUI plays it.
//!
//! The console's `audioserver` owns the sound hardware. A program that wants to
//! play does not take the device from it; it opens the OSS device `/dev/dsp`,
//! which a preloaded shim (`libpadsp`) answers on the server's behalf. That is
//! how the launcher's own sound works, and the shim is already loaded into every
//! process the launcher starts, this one included. There is no such file on the
//! card's filesystem: without the shim the open simply fails and the caller
//! falls back to something else.
//!
//! The other way in is the vendor library, which sets the hardware up itself.
//! That needs the server out of the way, and the tester's report was that it
//! stays refused even after the server is killed (`MI_AO_SetPubAttr` answers
//! `0xa0052009`), so this path does not touch the server at all. It also leaves
//! the volume where the console's own controls put it.
//!
//! What is not known: how the shim behaves beyond the plain OSS calls (does a
//! write block, which rates it accepts). So every step is negotiated and logged,
//! and the rate the device agrees to is the rate the ring runs at.

use std::ffi::{c_int, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::miao::raise_priority;
use super::pacer::Pacer;
use super::ring::Ring;
use super::silence::Silence;
use super::sink::{AudioError, AudioSink};

const DEVICE: &str = "/dev/dsp";

/// Frames in each write: 21 ms at 48 kHz.
const CHUNK: usize = 1024;
const CHANNELS: usize = 2;

/// How far ahead of real time the thread lets itself get before it sleeps. The
/// device blocks a writer whose buffer is full, but that is the thing not known
/// about the shim, so a clock keeps the thread honest either way.
const AHEAD: Duration = Duration::from_millis(60);

/// Behind real time by this much, the clock starts again from now.
const RESYNC: Duration = Duration::from_millis(100);

const RELEASE_AFTER: Duration = Duration::from_secs(3);
const RETRY_AFTER: Duration = Duration::from_secs(1);

/// The writes and stalls worth a line each, so a report shows how the device
/// paced them without printing one for every 21 ms for as long as a game runs.
const LOUD_WRITES: u32 = 6;
const SUMMARY_EVERY: Duration = Duration::from_secs(10);
const LOUD_SUMMARIES: u32 = 4;

const MIN_RATE: u32 = 8_000;
const MAX_RATE: u32 = 192_000;

/// OSS request numbers: `_IOWR('P', n, int)` on Linux.
const fn iowr_int(n: u32) -> u32 {
    (3 << 30) | (4 << 16) | ((b'P' as u32) << 8) | n
}
const SNDCTL_DSP_STEREO: u32 = iowr_int(3);
const SNDCTL_DSP_SPEED: u32 = iowr_int(2);
const SNDCTL_DSP_SETFMT: u32 = iowr_int(5);
const SNDCTL_DSP_CHANNELS: u32 = iowr_int(6);
const SNDCTL_DSP_SETFRAGMENT: u32 = iowr_int(10);
const AFMT_S16_LE: c_int = 0x10;

/// Four fragments of 4096 bytes: what SDL asks for 1024 stereo frames.
const FRAGMENT: c_int = (4 << 16) | 12;

/// The calls that configure the device, so the order and the answers they need
/// can be tested without one.
trait Control {
    fn call(&mut self, request: u32, value: &mut c_int) -> Result<(), String>;
}

struct Fd(c_int);

impl Fd {
    fn open(path: &str) -> Result<Fd, String> {
        let path = std::ffi::CString::new(path).map_err(|e| e.to_string())?;
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_WRONLY) };
        match fd {
            -1 => Err(std::io::Error::last_os_error().to_string()),
            fd => Ok(Fd(fd)),
        }
    }

    /// Writes all of `bytes`, retrying a signal and waiting out a device that
    /// says it is busy, for a bounded time.
    fn write_all(&self, mut bytes: &[u8]) -> Result<(), String> {
        let began = Instant::now();
        while !bytes.is_empty() {
            let n = unsafe { libc::write(self.0, bytes.as_ptr() as *const c_void, bytes.len()) };
            if n > 0 {
                bytes = &bytes[n as usize..];
                continue;
            }
            // errno means nothing after a write that returned zero.
            if n == 0 {
                return Err("the device accepted no data".into());
            }
            let e = std::io::Error::last_os_error();
            match e.kind() {
                std::io::ErrorKind::Interrupted => {}
                std::io::ErrorKind::WouldBlock if began.elapsed() < Duration::from_millis(250) => {
                    std::thread::sleep(Duration::from_millis(2));
                }
                _ => return Err(e.to_string()),
            }
        }
        Ok(())
    }
}

impl Control for Fd {
    fn call(&mut self, request: u32, value: &mut c_int) -> Result<(), String> {
        let code = unsafe { libc::ioctl(self.0, request as _, value as *mut c_int) };
        match code {
            -1 => Err(std::io::Error::last_os_error().to_string()),
            _ => Ok(()),
        }
    }
}

impl Drop for Fd {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.0);
        }
    }
}

/// Sets 16-bit little-endian stereo at `wanted` Hz and returns the rate the
/// device actually agreed to. Channels fall back to the older stereo switch for
/// a device that does not know the channel-count request.
fn negotiate(
    device: &mut dyn Control,
    wanted: u32,
    say: &mut dyn FnMut(String),
) -> Result<u32, String> {
    let mut format = AFMT_S16_LE;
    device
        .call(SNDCTL_DSP_SETFMT, &mut format)
        .map_err(|e| format!("setting the sample format failed: {e}"))?;
    if format != AFMT_S16_LE {
        return Err(format!(
            "the device chose sample format 0x{format:x}, not 16-bit"
        ));
    }
    say("format 16-bit little-endian".into());

    let mut channels = CHANNELS as c_int;
    match device.call(SNDCTL_DSP_CHANNELS, &mut channels) {
        Ok(()) if channels == CHANNELS as c_int => {}
        Ok(()) => {
            return Err(format!(
                "the device chose {channels} channel(s), not stereo"
            ))
        }
        Err(e) => {
            say(format!(
                "channel count refused ({e}); trying the stereo switch"
            ));
            let mut stereo = 1;
            device
                .call(SNDCTL_DSP_STEREO, &mut stereo)
                .map_err(|e| format!("setting stereo failed: {e}"))?;
            if stereo != 1 {
                return Err("the device would not play stereo".into());
            }
        }
    }
    say("2 channels".into());

    let mut rate = wanted as c_int;
    device
        .call(SNDCTL_DSP_SPEED, &mut rate)
        .map_err(|e| format!("setting the rate failed: {e}"))?;
    let rate = rate.max(0) as u32;
    if !(MIN_RATE..=MAX_RATE).contains(&rate) {
        return Err(format!("the device chose {rate} Hz"));
    }
    say(format!("rate {rate} Hz (asked for {wanted})"));

    // Only a hint, and not every OSS device implements it.
    let mut fragment = FRAGMENT;
    match device.call(SNDCTL_DSP_SETFRAGMENT, &mut fragment) {
        Ok(()) => say(format!("fragments set, answered 0x{fragment:x}")),
        Err(e) => say(format!(
            "fragment size not set ({e}); using the device's own"
        )),
    }
    Ok(rate)
}

fn span(frames: u64, rate: u32) -> Duration {
    Duration::from_secs_f64(frames as f64 / rate as f64)
}

/// How much audio has been written against how much real time has passed.
struct Clock {
    start: Instant,
    frames: u64,
}

/// Counts a written chunk and says how long to sleep so the thread stays no more
/// than `AHEAD` in front of real time.
fn ahead(clock: &mut Option<Clock>, now: Instant, frames: usize, rate: u32) -> Duration {
    let c = clock.get_or_insert(Clock {
        start: now,
        frames: 0,
    });
    c.frames += frames as u64;
    let sent = span(c.frames, rate);
    let elapsed = now.saturating_duration_since(c.start);
    if elapsed > sent + RESYNC {
        *clock = Some(Clock {
            start: now,
            frames: 0,
        });
        return Duration::ZERO;
    }
    sent.saturating_sub(elapsed).saturating_sub(AHEAD)
}

fn bytes_of(samples: &[i16]) -> &[u8] {
    // i16 has no padding and any byte is a valid u8; the device wants the same
    // little-endian layout this target stores them in.
    unsafe { std::slice::from_raw_parts(samples.as_ptr() as *const u8, samples.len() * 2) }
}

/// Opens the device and agrees a format with it, logging each step.
fn start(wanted: u32) -> Result<(Fd, u32), String> {
    let mut fd = Fd::open(DEVICE).map_err(|e| format!("cannot open {DEVICE}: {e}"))?;
    eprintln!("slot: audio: oss opened {DEVICE}");
    let rate = negotiate(&mut fd, wanted, &mut |line| {
        eprintln!("slot: audio: oss {line}")
    })?;
    Ok((fd, rate))
}

struct Writer {
    fd: Option<Fd>,
    rate: u32,
    clock: Option<Clock>,
    written: u64,
    chunks: u32,
    stalled: Duration,
    longest: Duration,
    summarised: Instant,
    summaries: u32,
    failing: bool,
    retry_at: Option<Instant>,
}

impl Writer {
    fn new(fd: Fd, rate: u32) -> Self {
        Writer {
            fd: Some(fd),
            rate,
            clock: None,
            written: 0,
            chunks: 0,
            stalled: Duration::ZERO,
            longest: Duration::ZERO,
            summarised: Instant::now(),
            summaries: 0,
            failing: false,
            retry_at: None,
        }
    }

    /// Writes one chunk and says how long to wait before the next.
    fn send(&mut self, samples: &[i16]) -> Result<Duration, String> {
        let began = Instant::now();
        let Some(fd) = self.fd.as_ref() else {
            return Err("the device is closed".into());
        };
        fd.write_all(bytes_of(samples))?;
        let took = began.elapsed();
        self.chunks += 1;
        self.written += (samples.len() / CHANNELS) as u64;
        self.stalled += took;
        self.longest = self.longest.max(took);
        if self.chunks <= LOUD_WRITES {
            eprintln!(
                "slot: audio: oss write {} of {} bytes took {:.1} ms",
                self.chunks,
                samples.len() * 2,
                took.as_secs_f64() * 1000.0
            );
        }
        if self.summaries < LOUD_SUMMARIES && self.summarised.elapsed() >= SUMMARY_EVERY {
            eprintln!(
                "slot: audio: oss {} chunks, {} frames, blocked {:.0} ms in all, longest {:.1} ms",
                self.chunks,
                self.written,
                self.stalled.as_secs_f64() * 1000.0,
                self.longest.as_secs_f64() * 1000.0
            );
            self.summarised = Instant::now();
            self.summaries += 1;
        }
        Ok(ahead(
            &mut self.clock,
            Instant::now(),
            samples.len() / CHANNELS,
            self.rate,
        ))
    }

    fn run(&mut self, ring: &Ring, stop: &AtomicBool, wanted: u32) {
        let mut buf = vec![0i16; CHUNK * CHANNELS];
        let mut silence = Silence::quiet(RELEASE_AFTER);
        let mut pace = Pacer::new(span(CHUNK as u64, self.rate));
        while !stop.load(Ordering::Relaxed) {
            ring.fill(&mut buf);
            let hold = silence.hear(&buf, span(CHUNK as u64, self.rate));
            if hold && self.retry_at.is_none_or(|at| Instant::now() >= at) {
                if self.retry_at.take().is_some() {
                    match start(wanted) {
                        Ok((fd, rate)) => {
                            self.fd = Some(fd);
                            self.failing = false;
                            self.clock = None;
                            if rate != self.rate {
                                self.rate = rate;
                                ring.reopen(rate);
                                pace = Pacer::new(span(CHUNK as u64, rate));
                            }
                            eprintln!("slot: audio: oss is back");
                        }
                        Err(e) => {
                            eprintln!("slot: audio: oss could not restart: {e}");
                            self.retry_at = Some(Instant::now() + RETRY_AFTER);
                            std::thread::sleep(pace.wait(Instant::now()));
                            continue;
                        }
                    }
                }
                pace.reset();
                match self.send(&buf) {
                    Ok(wait) if wait.is_zero() => {}
                    Ok(wait) => std::thread::sleep(wait),
                    Err(e) => {
                        if !self.failing {
                            eprintln!("slot: audio: oss write failed: {e}; reopening");
                            self.failing = true;
                        }
                        // Closed before it is opened again, in case the device
                        // will not have two writers.
                        self.fd = None;
                        self.clock = None;
                        self.retry_at = Some(Instant::now() + RETRY_AFTER);
                    }
                }
            } else {
                self.clock = None;
                std::thread::sleep(pace.wait(Instant::now()));
            }
        }
    }
}

pub struct OssSink {
    ring: Arc<Ring>,
    thread: Option<(Arc<AtomicBool>, JoinHandle<()>)>,
}

impl OssSink {
    pub fn new() -> Self {
        OssSink {
            ring: Arc::new(Ring::new(0)),
            thread: None,
        }
    }

    fn close(&mut self) {
        if let Some((stop, join)) = self.thread.take() {
            stop.store(true, Ordering::Relaxed);
            let _ = join.join();
        }
    }
}

impl Default for OssSink {
    fn default() -> Self {
        OssSink::new()
    }
}

impl Drop for OssSink {
    fn drop(&mut self) {
        self.close();
    }
}

impl AudioSink for OssSink {
    fn open(&mut self, sample_rate: u32) -> Result<(), AudioError> {
        self.close();
        let ring = self.ring.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let (ready_tx, ready_rx) = mpsc::channel();
        let join = std::thread::Builder::new()
            .name("slot-audio".into())
            .spawn(move || match start(sample_rate) {
                Ok((fd, rate)) => {
                    raise_priority();
                    ring.reopen(rate);
                    let _ = ready_tx.send(Ok(rate));
                    Writer::new(fd, rate).run(&ring, &flag, sample_rate);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            })
            .map_err(|e| AudioError::Device(e.to_string()))?;
        match ready_rx.recv() {
            Ok(Ok(rate)) => {
                eprintln!("slot: audio: oss ready at {rate} Hz");
                self.thread = Some((stop, join));
                Ok(())
            }
            Ok(Err(e)) => {
                let _ = join.join();
                Err(AudioError::Device(e))
            }
            Err(_) => {
                let _ = join.join();
                Err(AudioError::Device("output thread stopped".into()))
            }
        }
    }

    fn ring(&self) -> Arc<Ring> {
        self.ring.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A device that answers from a table, and records what it was asked.
    struct Fake {
        asked: Vec<u32>,
        answers: Vec<(u32, Result<c_int, &'static str>)>,
    }

    impl Fake {
        fn new(answers: Vec<(u32, Result<c_int, &'static str>)>) -> Self {
            Fake {
                asked: Vec::new(),
                answers,
            }
        }
    }

    impl Control for Fake {
        fn call(&mut self, request: u32, value: &mut c_int) -> Result<(), String> {
            self.asked.push(request);
            match self.answers.iter().find(|(r, _)| *r == request) {
                Some((_, Ok(v))) => {
                    *value = *v;
                    Ok(())
                }
                Some((_, Err(e))) => Err(e.to_string()),
                None => Ok(()),
            }
        }
    }

    fn quiet(_: String) {}

    #[test]
    fn request_numbers_are_the_oss_ones() {
        assert_eq!(SNDCTL_DSP_SPEED, 0xC004_5002);
        assert_eq!(SNDCTL_DSP_STEREO, 0xC004_5003);
        assert_eq!(SNDCTL_DSP_SETFMT, 0xC004_5005);
        assert_eq!(SNDCTL_DSP_CHANNELS, 0xC004_5006);
        assert_eq!(SNDCTL_DSP_SETFRAGMENT, 0xC004_500A);
    }

    #[test]
    fn it_asks_format_then_channels_then_rate() {
        let mut dev = Fake::new(vec![
            (SNDCTL_DSP_SETFMT, Ok(AFMT_S16_LE)),
            (SNDCTL_DSP_CHANNELS, Ok(2)),
            (SNDCTL_DSP_SPEED, Ok(48_000)),
        ]);
        assert_eq!(negotiate(&mut dev, 48_000, &mut quiet), Ok(48_000));
        assert_eq!(
            dev.asked,
            vec![
                SNDCTL_DSP_SETFMT,
                SNDCTL_DSP_CHANNELS,
                SNDCTL_DSP_SPEED,
                SNDCTL_DSP_SETFRAGMENT
            ]
        );
    }

    #[test]
    fn the_rate_the_device_agrees_to_is_the_rate_used() {
        let mut dev = Fake::new(vec![
            (SNDCTL_DSP_SETFMT, Ok(AFMT_S16_LE)),
            (SNDCTL_DSP_CHANNELS, Ok(2)),
            (SNDCTL_DSP_SPEED, Ok(44_100)),
        ]);
        assert_eq!(negotiate(&mut dev, 48_000, &mut quiet), Ok(44_100));
    }

    #[test]
    fn a_device_that_will_not_do_16_bit_is_refused() {
        let mut dev = Fake::new(vec![(SNDCTL_DSP_SETFMT, Ok(0x8))]);
        assert!(negotiate(&mut dev, 48_000, &mut quiet)
            .unwrap_err()
            .contains("not 16-bit"));
    }

    #[test]
    fn a_failed_format_request_is_refused() {
        let mut dev = Fake::new(vec![(SNDCTL_DSP_SETFMT, Err("bad ioctl"))]);
        assert!(negotiate(&mut dev, 48_000, &mut quiet)
            .unwrap_err()
            .contains("bad ioctl"));
    }

    #[test]
    fn mono_only_is_refused() {
        let mut dev = Fake::new(vec![
            (SNDCTL_DSP_SETFMT, Ok(AFMT_S16_LE)),
            (SNDCTL_DSP_CHANNELS, Ok(1)),
        ]);
        assert!(negotiate(&mut dev, 48_000, &mut quiet)
            .unwrap_err()
            .contains("not stereo"));
    }

    #[test]
    fn the_older_stereo_switch_is_the_fallback_for_channels() {
        let mut dev = Fake::new(vec![
            (SNDCTL_DSP_SETFMT, Ok(AFMT_S16_LE)),
            (SNDCTL_DSP_CHANNELS, Err("not supported")),
            (SNDCTL_DSP_STEREO, Ok(1)),
            (SNDCTL_DSP_SPEED, Ok(48_000)),
        ]);
        assert_eq!(negotiate(&mut dev, 48_000, &mut quiet), Ok(48_000));
        assert!(dev.asked.contains(&SNDCTL_DSP_STEREO));
    }

    #[test]
    fn a_refused_stereo_switch_ends_it() {
        let mut dev = Fake::new(vec![
            (SNDCTL_DSP_SETFMT, Ok(AFMT_S16_LE)),
            (SNDCTL_DSP_CHANNELS, Err("no")),
            (SNDCTL_DSP_STEREO, Err("no")),
        ]);
        assert!(negotiate(&mut dev, 48_000, &mut quiet).is_err());
    }

    #[test]
    fn a_nonsense_rate_is_refused() {
        for bad in [0, 1_000, 500_000] {
            let mut dev = Fake::new(vec![
                (SNDCTL_DSP_SETFMT, Ok(AFMT_S16_LE)),
                (SNDCTL_DSP_CHANNELS, Ok(2)),
                (SNDCTL_DSP_SPEED, Ok(bad)),
            ]);
            assert!(negotiate(&mut dev, 48_000, &mut quiet).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_device_without_fragment_support_still_works() {
        let mut dev = Fake::new(vec![
            (SNDCTL_DSP_SETFMT, Ok(AFMT_S16_LE)),
            (SNDCTL_DSP_CHANNELS, Ok(2)),
            (SNDCTL_DSP_SPEED, Ok(48_000)),
            (SNDCTL_DSP_SETFRAGMENT, Err("not supported")),
        ]);
        assert_eq!(negotiate(&mut dev, 48_000, &mut quiet), Ok(48_000));
    }

    #[test]
    fn the_steps_are_logged() {
        let mut lines = Vec::new();
        let mut dev = Fake::new(vec![
            (SNDCTL_DSP_SETFMT, Ok(AFMT_S16_LE)),
            (SNDCTL_DSP_CHANNELS, Ok(2)),
            (SNDCTL_DSP_SPEED, Ok(48_000)),
        ]);
        negotiate(&mut dev, 48_000, &mut |l| lines.push(l)).unwrap();
        assert!(lines.iter().any(|l| l.contains("rate 48000 Hz")));
        assert!(lines.iter().any(|l| l.contains("fragments set")));
    }

    #[test]
    fn a_device_that_takes_data_instantly_is_slowed_to_real_time() {
        let mut clock = None;
        let t0 = Instant::now();
        let mut wait = Duration::ZERO;
        // Ten chunks handed over in no time at all.
        for _ in 0..10 {
            wait = ahead(&mut clock, t0, CHUNK, 48_000);
        }
        // 10 x 21.3 ms is 213 ms sent; more than AHEAD of it has to be waited out.
        let sent = span(10 * CHUNK as u64, 48_000);
        assert_eq!(wait, sent - AHEAD);
    }

    #[test]
    fn a_device_that_blocks_in_step_is_not_slowed() {
        let mut clock = None;
        let t0 = Instant::now();
        let mut now = t0;
        for _ in 0..20 {
            now += span(CHUNK as u64, 48_000);
            assert_eq!(ahead(&mut clock, now, CHUNK, 48_000), Duration::ZERO);
        }
    }

    #[test]
    fn falling_far_behind_restarts_the_clock_instead_of_catching_up() {
        let mut clock = None;
        let t0 = Instant::now();
        ahead(&mut clock, t0, CHUNK, 48_000);
        let late = t0 + Duration::from_secs(2);
        assert_eq!(ahead(&mut clock, late, CHUNK, 48_000), Duration::ZERO);
        // And the next chunk is measured from there, not from the old start.
        let next = ahead(&mut clock, late, CHUNK, 48_000);
        assert_eq!(next, Duration::ZERO);
        assert_eq!(clock.as_ref().unwrap().start, late);
    }

    #[test]
    fn samples_are_written_as_little_endian_pairs() {
        let bytes = bytes_of(&[0x0102, -2]);
        assert_eq!(bytes, &[0x02, 0x01, 0xFE, 0xFF]);
    }

    fn pipe() -> (Fd, c_int) {
        let mut ends = [0 as c_int; 2];
        assert_eq!(unsafe { libc::pipe(ends.as_mut_ptr()) }, 0);
        (Fd(ends[1]), ends[0])
    }

    #[test]
    fn what_is_written_reaches_the_other_end_whole() {
        let (writer, reader) = pipe();
        let data: Vec<u8> = (0..4096u32).map(|n| n as u8).collect();
        writer.write_all(&data).unwrap();
        let mut got = vec![0u8; data.len()];
        let mut at = 0;
        while at < got.len() {
            let n = unsafe {
                libc::read(
                    reader,
                    got[at..].as_mut_ptr() as *mut c_void,
                    got.len() - at,
                )
            };
            assert!(n > 0);
            at += n as usize;
        }
        assert_eq!(got, data);
        unsafe { libc::close(reader) };
    }

    #[test]
    fn a_closed_far_end_is_an_error_not_a_hang() {
        let (writer, reader) = pipe();
        unsafe { libc::close(reader) };
        // The write would raise SIGPIPE; ignore it for the test the way the
        // frontend does, so the error comes back as a value.
        unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN) };
        assert!(writer.write_all(&[0; 64]).is_err());
    }

    #[test]
    fn a_file_that_is_not_a_sound_device_refuses_the_requests() {
        let (mut not_dsp, reader) = pipe();
        let mut value = AFMT_S16_LE;
        assert!(not_dsp.call(SNDCTL_DSP_SETFMT, &mut value).is_err());
        assert!(negotiate(&mut not_dsp, 48_000, &mut quiet).is_err());
        unsafe { libc::close(reader) };
    }

    #[test]
    fn opening_a_device_that_is_not_there_is_an_error_not_a_panic() {
        assert!(Fd::open("/nonexistent/slot-dsp").is_err());
    }
}
