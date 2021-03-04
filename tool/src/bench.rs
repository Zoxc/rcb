use crate::Build;
use crate::OnDrop;
use crate::State;
use clap::ArgMatches;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde_derive::Deserialize;
use std::{
    fs,
    path::Path,
    path::PathBuf,
    process::{Command, Stdio},
    sync::Arc,
};

#[derive(Deserialize)]
struct BenchToml {
    cargo_dir: Option<String>,
}

struct Bench {
    name: String,
    cargo_dir: PathBuf,
}

struct BenchConfig {
    incremental: bool,
    mode: BenchMode,
}

enum BenchMode {
    Check,
    Debug,
    Release,
}

struct Config {
    session_dir: PathBuf,
    state: Arc<State>,
    build: String,
    bench: Arc<Bench>,
}

impl Config {
    fn display(&self) -> String {
        format!("benchmark `{}` with build ``", self.bench.name, self.build)
    }

    fn path(&self) -> PathBuf {
        self.session_dir.join(&self.build).join(&self.bench.name)
    }

    fn prepare(&self) {
        t!(fs::create_dir_all(self.path()));

        let mut output = Command::new("cargo");
        output
            .current_dir(&self.bench.cargo_dir)
            .stdin(Stdio::null())
            .env(
                "RUSTC",
                self.state
                    .root
                    .join("builds")
                    .join(&self.build)
                    .join("stage1")
                    .join("bin")
                    .join("rustc"),
            )
            .env("RUSTFLAGS", "-Ztime")
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_TARGET_DIR", self.path().join("target"))
            .arg("check");
        //.arg("-vv");

        println!("cargo {:#?}", output);

        let output = t!(output.output());

        if !output.status.success() {
            //println!(output.stdout.)
            panic!(
                "Unable to prepare config - build:{} bench:{}",
                self.build, self.bench.name
            );
        }

        println!("Prepared {}", self.display());
    }
}

pub fn bench(state: Arc<State>, matches: &ArgMatches) {
    let builds: Vec<Build> = matches
        .values_of("BUILD")
        .unwrap()
        .enumerate()
        .map(|(i, build_name)| {
            let build_path = state.root.join("builds").join(build_name);
            let build = build_path.join("build.toml");
            if !build.exists() {
                panic!("Cannot find build `{}`", build_name);
            }
            let build = t!(fs::read_to_string(build));
            let build: Build = t!(toml::from_str(&build));
            println!(
                "Build #{} {} ({} {})",
                i + 1,
                build_name,
                build.commit.as_deref().unwrap_or(""),
                build.size_display
            );
            build
        })
        .collect();

    let benchs: Vec<Arc<Bench>> = t!(fs::read_dir(state.root.join("benchs")))
        .filter_map(|f| {
            let f = t!(f);
            let path = f.path();
            let name = path.file_name().unwrap();
            if t!(f.file_type()).is_dir() {
                let info = t!(fs::read_to_string(path.join("bench.toml")));
                let info: BenchToml = t!(toml::from_str(&info));
                println!("Benchmark {}", path.display());
                let name = name.to_string_lossy().into_owned();
                Some(Arc::new(Bench {
                    name,
                    cargo_dir: path.join(info.cargo_dir.unwrap_or(".".to_owned())),
                }))
            } else {
                None
            }
        })
        .collect();

    t!(fs::create_dir_all(state.root.join("tmp")));
    let session_dir = crate::temp_dir(&state.root.join("tmp"));

    let session_dir2 = session_dir.clone();
    let state2 = state.clone();
    let _drop_session_dir = OnDrop(move || {
        //crate::remove_recursively(&session_dir2);
        fs::remove_dir(state2.root.join("tmp")).ok();
    });

    let configs: Vec<Config> = builds
        .iter()
        .flat_map(|build| {
            let state = state.clone();
            let benchs: Vec<_> = benchs
                .iter()
                .map(|bench| Config {
                    session_dir: session_dir.clone(),
                    state: state.clone(),
                    build: build.name.clone(),
                    bench: bench.clone(),
                })
                .collect();
            benchs
        })
        .collect();

    configs.par_iter().for_each(|config| {
        config.prepare();
    })
}
