macro_rules! eprintln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

#[cfg(not(feature = "host"))]
mod device_app;
#[cfg(feature = "host")]
mod host_app;

fn main() {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("--init-root") {
        let Some(dir) = args.next() else {
            eprintln!("slot: --init-root needs a directory");
            std::process::exit(2);
        };
        slot::root::ensure(std::path::Path::new(&dir));
        return;
    }
    slot::crash::install();

    // The Mini Plus panel is mounted upside down. The launcher sets this, but a
    // tester can end up with a new binary beside an old launcher, so the
    // binary supplies the answer itself unless something already chose one.
    // Nothing else is running yet, so changing the environment is safe.
    #[cfg(feature = "miyoomini")]
    if std::env::var_os("SLOT_FB_ROTATE").is_none() {
        std::env::set_var("SLOT_FB_ROTATE", "180");
    }

    // First line of every log, so a report from a handheld can be matched to a
    // build without guessing from the rest of the output. The Mini Plus test
    // builds are why this exists: one report turned out to be the previous
    // package, and nothing in the log said so.
    let build = slot::build_info::Build::current();
    eprintln!(
        "slot: {} ({}) built {} for {}",
        build.version,
        build.hash,
        build.date,
        slot::build_info::DEVICE
    );

    #[cfg(feature = "host")]
    host_app::run();

    #[cfg(not(feature = "host"))]
    device_app::run();
}
