use crate::Build;
use crate::OnDrop;
use crate::State;
use clap::ArgMatches;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::Arc,
};

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
    bench: String,
}

impl Config {
    fn path(&self) -> PathBuf {
        self.session_dir.join(&self.build).join(&self.bench)
    }

    fn prepare(&self) {
        t!(fs::create_dir_all(self.path()));

        let mut output = Command::new("cargo");
        output
            .current_dir(self.state.root.join("benchs").join(&self.bench))
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
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_TARGET_DIR", self.path().join("target"))
            .arg("rustc")
            .arg("-vv");

        println!("cargo {:#?}", output);

        output.status().ok();
        /*   .output()
            .expect("failed to execute process");

        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        } else {
            None
        }*/
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

    let benchs: Vec<_> = t!(fs::read_dir(state.root.join("benchs")))
        .filter_map(|f| {
            let f = t!(f);
            let path = f.path();
            let name = path.file_name().unwrap();
            if t!(f.file_type()).is_dir() {
                println!("Benchmark {}", path.display());
                let name = name.to_string_lossy().into_owned();
                Some((name, path))
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
                    bench: bench.0.clone(),
                })
                .collect();
            benchs
        })
        .collect();

    configs.par_iter().for_each(|config| {
        config.prepare();
    })
}
