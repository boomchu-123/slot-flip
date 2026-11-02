#[cfg(all(target_os = "linux", feature = "miyoomini"))]
pub fn pin_current(cpu: usize, role: &str) {
    let mut set = unsafe { std::mem::zeroed::<libc::cpu_set_t>() };
    unsafe {
        libc::CPU_ZERO(&mut set);
        libc::CPU_SET(cpu, &mut set);
    }
    let result = unsafe {
        libc::sched_setaffinity(
            0,
            std::mem::size_of::<libc::cpu_set_t>(),
            &set as *const libc::cpu_set_t,
        )
    };
    if result == 0 {
        eprintln!("slot: cpu: {role} pinned to CPU {cpu}");
    } else {
        eprintln!(
            "slot: cpu: could not pin {role} to CPU {cpu}: {}",
            std::io::Error::last_os_error()
        );
    }
}

#[cfg(not(all(target_os = "linux", feature = "miyoomini")))]
pub fn pin_current(_cpu: usize, _role: &str) {}
