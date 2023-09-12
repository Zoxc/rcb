use std::{env, process::Command, time::Instant};
#[cfg(windows)]
use {std::os::windows::process::CommandExt, winapi::um::winbase::HIGH_PRIORITY_CLASS};

pub fn run() -> ! {
    let arguments: Vec<_> = env::args_os().collect();
    let mut output = Command::new(&arguments[1]);
    for argument in &arguments[2..] {
        output.arg(argument);
    }

    #[cfg(windows)]
    output.creation_flags(HIGH_PRIORITY_CLASS);

    if env::var("RCB_TIME_DETAILS").is_ok() {
        output.arg("-Ztime-passes");
        output.arg("-Ztime-passes-format=json");
    }

    let start = Instant::now();

    let prefix = env::var("RCB_TIME_PREFIX").ok();

    let status = output.status().expect("failed to execute the real rustc");

    let duration = start.elapsed();

    prefix.map(|prefix| eprintln!("{}{}", prefix, duration.as_micros()));

    std::process::exit(status.code().unwrap_or(-1));
}
