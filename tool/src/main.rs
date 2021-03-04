use clap::SubCommand;
use clap::{App, AppSettings, Arg};
use serde_derive::Deserialize;
use toml;

#[derive(Deserialize)]
struct Config {
    ip: String,
}

fn main() {
    let fetch = SubCommand::with_name("fetch")
        .arg(Arg::with_name("ref").long("ref"))
        .arg(Arg::with_name("REPO"));
    let bench = SubCommand::with_name("bench").arg(Arg::with_name("REPO-OR-BUILD").multiple(true));
    let matches = App::new("rcb")
        .about("Rust Compiler Bencher")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(fetch)
        .subcommand(bench)
        .get_matches();

        let config_path = std::env::current_exe().unwrap().parent().unwrap().join("rcb.toml");

        let config: Config = match std::fs::read_to_string(&config_path) {
            Ok(config) => match toml::from_str(&config) {
                    Ok(config) => config,
                    Err(err) => panic!("Unable to parse configuration file at {}, error: {}", config_path.display(), err),
            }
            Err(_) => panic!("Unable to read configuration file at {}", config_path.display()),
        };

}
