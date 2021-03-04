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
    time::Instant,
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

pub fn remove_fingerprint(path: &Path, krate: &str) {
    for f in t!(fs::read_dir(path)) {
        let f = t!(f);
        let path = f.path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if let Some(i) = name.as_bytes().iter().rposition(|c| *c == '-' as u8) {
            if &name[0..i] == krate {
                crate::remove_recursively(&path);
                return;
            }
        }
    }
    panic!("Didn't find fingerprint for {}", krate);
}

struct Config {
    session_dir: PathBuf,
    state: Arc<State>,
    build: String,
    bench: Arc<Bench>,
    time: Vec<f64>,
}

impl Config {
    fn display(&self) -> String {
        format!(
            "benchmark `{}` with build `{}`",
            self.bench.name, self.build
        )
    }

    fn path(&self) -> PathBuf {
        self.session_dir.join(&self.build).join(&self.bench.name)
    }

    fn prepare(&self) {
        println!("Preparing {}", self.display());

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

    fn run(&mut self) {
        let fingerprint_dir = self
            .path()
            .join("target")
            .join("debug")
            .join(".fingerprint");

        remove_fingerprint(&fingerprint_dir, &self.bench.name);

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

        let start = Instant::now();
        let output = t!(output.output());
        let duration = start.elapsed();

        if !output.status.success() {
            //println!(output.stdout.)
            panic!(
                "Unable to run - build:{} bench:{}",
                self.build, self.bench.name
            );
        }

        let stderr = t!(std::str::from_utf8(&output.stderr));

        println!("stderr = {}", stderr);

        println!("Ran {} in {:?}", self.display(), duration);

        self.time.push(duration.as_secs_f64());
    }

    fn summary(&mut self) {
        println!(
            "Average for {} = {:.06}s",
            self.display(),
            self.time.iter().map(|t| *t).sum::<f64>() / self.time.len() as f64
        );
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

    let mut benchs: Vec<Arc<Bench>> = t!(fs::read_dir(state.root.join("benchs")))
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

    benchs.pop();

    t!(fs::create_dir_all(state.root.join("tmp")));
    let session_dir = crate::temp_dir(&state.root.join("tmp"));

    let session_dir2 = session_dir.clone();
    let state2 = state.clone();
    let _drop_session_dir = OnDrop(move || {
        //crate::remove_recursively(&session_dir2);
        fs::remove_dir(state2.root.join("tmp")).ok();
    });

    let mut configs: Vec<Vec<Config>> = benchs
        .iter()
        .map(|bench| {
            builds
                .iter()
                .map(|build| Config {
                    time: Vec::new(),
                    session_dir: session_dir.clone(),
                    state: state.clone(),
                    build: build.name.clone(),
                    bench: bench.clone(),
                })
                .collect()
        })
        .collect();

    configs.par_iter().for_each(|builds| {
        builds.par_iter().for_each(|config| {
            config.prepare();
        });
    });

    for builds in &mut configs {
        for _ in 0..3 {
            for build in &mut *builds {
                println!("Benching {}", build.display());
                build.run();
            }
        }
        for build in builds {
            build.summary();
        }
    }
}
