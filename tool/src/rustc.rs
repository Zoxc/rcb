use std::{env, process::Command, time::Instant};

pub fn run() -> ! {
    let arguments: Vec<_> = env::args_os().collect();
    let mut output = Command::new(&arguments[1]);
    for argument in &arguments[2..] {
        output.arg(argument);
    }
    let start = Instant::now();

    let status = output.status().expect("failed to execute the real rustc");

    let duration = start.elapsed();

    eprintln!("rcb-rustc-timer:{}", duration.as_micros());

    std::process::exit(status.code().unwrap_or(-1));
}
