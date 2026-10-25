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
