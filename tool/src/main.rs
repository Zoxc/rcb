use clap::SubCommand;
use clap::{App, AppSettings, Arg};
use serde_derive::Deserialize;
use std::{collections::HashMap, path::PathBuf};
use toml;

mod fetch;

#[derive(Deserialize, Debug)]
struct Repo {
    path: PathBuf,
    default: Option<bool>,
}

#[derive(Deserialize, Debug)]
struct Config {
    root: Option<PathBuf>,
    repo: HashMap<String, Repo>,
}

#[derive(Debug)]
pub struct State {
    config: Config,
}

impl State {
    fn repo_path(&self, name: &str) -> PathBuf {
        self.config.repo.get(name).unwrap().path.clone()
    }

    fn repo(&self, name: String) -> String {
        self.config
            .repo
            .get(&name)
            .unwrap_or_else(|| panic!("Repository `{}` doesn't exist", name));
        name
    }

    fn default_repo(&self) -> String {
        let defaults: Vec<_> = self
            .config
            .repo
            .iter()
            .filter(|(_, v)| v.default.unwrap_or_default())
            .map(|(k, _)| k.clone())
            .collect();
        match defaults[..] {
            [] => panic!("No default repository configured"),
            [ref default] => default.to_owned(),
            _ => panic!("Error, multiple default repositories: {:?}", defaults),
        }
    }
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

    let exe_path = std::env::current_exe().unwrap();
    let exe_path = exe_path.parent().unwrap();
    let config_path = exe_path.join("rcb.toml");

    let config: Config = match std::fs::read_to_string(&config_path) {
        Ok(config) => match toml::from_str(&config) {
            Ok(config) => config,
            Err(err) => panic!(
                "Unable to parse configuration file at {}, error: {}",
                config_path.display(),
                err
            ),
        },
        Err(_) => panic!(
            "Unable to read configuration file at {}",
            config_path.display()
        ),
    };

    let root = config.root.as_ref().map(|root| &**root).unwrap_or(exe_path);

    println!("Root is {}", root.display());
    println!("Config {:#?}", config);

    let state = State { config };

    if let Some(matches) = matches.subcommand_matches("fetch") {
        fetch::fetch(state, matches);
    }
}
