use crate::bench::InstanceTime;
use std::{env, mem, os::windows::io::AsRawHandle, process::Command, time::Instant};

#[cfg(windows)]
use {
    std::os::windows::process::CommandExt,
    winapi::um::psapi::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
    winapi::um::winbase::HIGH_PRIORITY_CLASS,
};

pub fn run() -> ! {
    let arguments: Vec<_> = env::args_os().collect();
    let mut cmd = Command::new(&arguments[1]);
    for argument in &arguments[2..] {
        cmd.arg(argument);
    }

    #[cfg(windows)]
    cmd.creation_flags(HIGH_PRIORITY_CLASS);

    // Since rust-lang/rust#89836, rustc stable crate IDs include a hash of the
    // rustc version (including the git commit it's built from), which means
    // that hashmaps or other structures have different behavior when comparing
    // different rustc builds. This is bad for rustc-perf, as it means that
    // comparing two commits has a source of noise that makes it harder to know
    // what the actual change between two artifacts is.
    cmd.env("RUSTC_FORCE_INCR_COMP_ARTIFACT_HEADER", "rustc-rcb");
    cmd.env("RUSTC_FORCE_RUSTC_VERSION", "rustc-rcb");

    // There is another similar source of hashing noise. Cargo queries the version of rustc
    // using `rustc -vV`, and then hashes part of the output, and passes it to `rustc` using
    // `-Cmetadata`. This means that two different versions of rustc might have a different metadata
    // value, and thus different hash value.
    // However, for rustc-perf, this is mostly a non-issue, because for nightly releases, cargo
    // currently only hashes the host (which should stay the same, for the time being), and the part
    // of the rustc version after -, which should be "nightly" for all try builds and also master
    // commits.

    if env::var("RCB_TIME_DETAILS").is_ok() {
        cmd.arg("-Ztime-passes");
        cmd.arg("-Ztime-passes-format=json");
    }

    let start = Instant::now();

    let prefix = env::var("RCB_TIME_PREFIX").ok();

    let mut child = cmd.spawn().expect("failed to execute the real rustc");

    let status = child.wait().expect("failed to wait for the real rustc");

    let duration = start.elapsed();

    let mut time = InstanceTime {
        duration: duration.as_secs_f64(),
        peak_committed: None,
        peak_physical: None,
    };

    #[cfg(windows)]
    {
        unsafe {
            let handle = child.as_raw_handle();
            let mut counters: PROCESS_MEMORY_COUNTERS = mem::zeroed();
            counters.cb = mem::size_of_val(&counters) as u32;
            if GetProcessMemoryInfo(handle, &mut counters, mem::size_of_val(&counters) as u32) != 0
            {
                time.peak_committed = Some(counters.PeakPagefileUsage);
                time.peak_physical = Some(counters.PeakWorkingSetSize);
            }
        }
    }

    prefix.map(|prefix| eprintln!("\n{}{}", prefix, serde_json::to_string(&time).unwrap()));

    std::process::exit(status.code().unwrap_or(-1));
}
