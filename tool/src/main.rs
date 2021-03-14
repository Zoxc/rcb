use clap::SubCommand;
use clap::{App, AppSettings, Arg};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde_derive::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs, iter,
    path::{Path, PathBuf},
    sync::Arc,
};
use toml;

pub struct OnDrop<F: Fn()>(pub F);

impl<F: Fn()> OnDrop<F> {
    /// Forgets the function which prevents it from running.
    /// Ensure that the function owns no memory, otherwise it will be leaked.
    #[inline]
    pub fn disable(self) {
        std::mem::forget(self);
    }
}

impl<F: Fn()> Drop for OnDrop<F> {
    #[inline]
    fn drop(&mut self) {
        (self.0)();
    }
}

/// A helper macro to `unwrap` a result except also print out details like:
///
/// * The file/line of the panic
/// * The expression that failed
/// * The error itself
macro_rules! t {
    ($e:expr) => {
        match $e {
            Ok(e) => e,
            Err(e) => panic!("{} failed with {}", stringify!($e), e),
        }
    };
    // it can show extra info in the second parameter
    ($e:expr, $extra:expr) => {
        match $e {
            Ok(e) => e,
            Err(e) => panic!("{} failed with {} ({:?})", stringify!($e), e, $extra),
        }
    };
}

mod bench;
mod fetch;
mod rustc;

#[derive(Serialize, Deserialize, Debug)]
struct BuildFile {
    path: String,
    size: u64,
}

#[derive(Serialize, Deserialize, Debug)]
struct Build {
    name: String,
    path: String,
    repo: String,
    repo_path: PathBuf,
    branch: Option<String>,
    commit: Option<String>,
    commit_short: Option<String>,
    commit_title: Option<String>,
    upstream: Option<String>,
    upstream_short: Option<String>,
    upstream_title: Option<String>,
    size: u64,
    signature: String,
    triple: String,
    files: Vec<BuildFile>,
    config: toml::Value,
}

#[derive(Deserialize, Debug)]
struct Repo {
    path: PathBuf,
    default: Option<bool>,
}

#[derive(Deserialize, Debug)]
struct Config {
    iterations: Option<usize>,
    root: Option<PathBuf>,
    repo: HashMap<String, Repo>,
}

#[derive(Debug)]
pub struct State {
    exe: PathBuf,
    root: PathBuf,
    config: Config,
    verbose: bool,
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

fn remove_recursively(path: &Path) {
    if !path.exists() {
        return;
    }
    for f in t!(fs::read_dir(path)) {
        let f = t!(f);
        let path = f.path();
        if t!(f.file_type()).is_dir() {
            remove_recursively(&path);
        } else {
            t!(fs::remove_file(path));
        }
    }
    fs::remove_dir(path).ok();
}

fn temp_dir(parent: &Path) -> PathBuf {
    let mut attempts = 0;
    let mut rng = rand::thread_rng();
    loop {
        let temp_name: String = iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .map(char::from)
            .take(32)
            .collect();
        let tmp = parent.join(&temp_name).to_owned();
        if fs::create_dir(&tmp).is_ok() {
            return tmp;
        }
        attempts += 1;

        if attempts > 10 {
            panic!("Failed to create temporary directory");
        }
    }
}

fn main() {
    if std::env::var_os("RCB_ACT_AS_RUSTC").is_some() {
        rustc::run();
    }

    let fetch = SubCommand::with_name("fetch")
        .arg(Arg::with_name("ref").long("ref"))
        .arg(Arg::with_name("REPO"));
    let bench = SubCommand::with_name("bench")
        .arg(Arg::with_name("BUILD").multiple(true).required(true))
        .arg(
            Arg::with_name("bench")
                .multiple(true)
                .short("b")
                .long("bench")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("threads")
                .long("threads")
                .takes_value(true)
                .help("Don't pass -j1 to cargo"),
        )
        .arg(
            Arg::with_name("rflag")
                .long("rflag")
                .takes_value(true)
                .help("Arguments to rustc"),
        )
        .arg(
            Arg::with_name("cflag")
                .long("cflag")
                .takes_value(true)
                .help("Arguments to cargo"),
        )
        .arg(
            Arg::with_name("env")
                .long("env")
                .takes_value(true)
                .help("Enviroment variable to cargo"),
        )
        .arg(Arg::with_name("details").long("details").takes_value(true))
        .arg(Arg::with_name("iterations").short("n").takes_value(true))
        .arg(Arg::with_name("incr-none").long("incr-none"))
        .arg(Arg::with_name("incr-initial").long("incr-initial"))
        .arg(Arg::with_name("incr-unchanged").long("incr-unchanged"))
        .arg(Arg::with_name("check").long("check"))
        .arg(Arg::with_name("release").long("release"))
        .arg(Arg::with_name("debug").long("debug"));
    let matches = App::new("rcb")
        .about("Rust Compiler Bencher")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(fetch)
        .subcommand(bench)
        .get_matches();

    let exe = std::env::current_exe().unwrap();
    let exe_path = exe.parent().unwrap();
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

    let root = config
        .root
        .as_ref()
        .map(|root| &**root)
        .unwrap_or(exe_path)
        .to_owned();

    println!("Root is {}", root.display());

    let state = Arc::new(State {
        exe,
        root,
        config,
        verbose: false,
    });

    if let Some(matches) = matches.subcommand_matches("fetch") {
        fetch::fetch(state, matches);
    } else if let Some(matches) = matches.subcommand_matches("bench") {
        bench::bench(state, matches);
    }
}
