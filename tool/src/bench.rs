use crate::Build;
use crate::OnDrop;
use crate::State;
use clap::value_t;
use clap::ArgMatches;
use core::panic;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde_derive::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::Write,
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

#[derive(Clone)]
struct Config {
    incremental: bool,
    mode: BenchMode,
    bench: Arc<Bench>,
}

impl Config {
    fn display(&self) -> String {
        format!("{}:{}", self.bench.name, self.mode.display())
    }
}
#[derive(Clone, Copy)]
enum BenchMode {
    Check,
    Debug,
    Release,
}

impl BenchMode {
    fn display(&self) -> &'static str {
        match self {
            BenchMode::Check => "check",
            BenchMode::Debug => "debug",
            BenchMode::Release => "release",
        }
    }
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

#[derive(Serialize, Clone)]
struct TimeData {
    name: String,
    time: f64,
    before_rss: String,
    after_rss: String,
}

#[derive(Serialize)]
struct ResultConfig {
    build: String,
    time: Vec<f64>,
    times: Vec<Vec<TimeData>>,
}

#[derive(Serialize)]
struct ResultBench {
    name: String,
    builds: Vec<ResultConfig>,
}

#[derive(Serialize)]
struct Result {
    benchs: Vec<ResultBench>,
}

struct Instance {
    session_dir: PathBuf,
    state: Arc<State>,
    build: String,
    config: Config,
    time: Vec<f64>,
    times: Vec<Vec<TimeData>>,
}

struct ConfigInstances {
    config: Config,
    builds: Vec<Instance>,
}

impl Instance {
    fn display(&self) -> String {
        format!(
            "benchmark {} with build `{}`",
            self.config.display(),
            self.build
        )
    }

    fn path(&self) -> PathBuf {
        self.session_dir
            .join(&self.build)
            .join(&self.config.bench.name)
            .join(self.config.mode.display())
    }

    fn prepare(&self) {
        println!("Preparing {}", self.display());

        t!(fs::create_dir_all(self.path()));

        let mut output = Command::new("cargo");
        output
            .current_dir(&self.config.bench.cargo_dir)
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
            let stderr = t!(std::str::from_utf8(&output.stderr));
            let stdout = t!(std::str::from_utf8(&output.stdout));

            println!(
                "Unable to prepare {}\n\nSTDERR:\n{}\n\nSTDOUT:\n{}\n",
                self.display(),
                stderr,
                stdout
            );
            panic!("Unable to prepare instance");
        }

        println!("Prepared {}", self.display());
    }

    fn run(&mut self, warmup: bool) {
        let fingerprint_dir = self
            .path()
            .join("target")
            .join("debug")
            .join(".fingerprint");

        remove_fingerprint(&fingerprint_dir, &self.config.bench.name);

        let mut output = Command::new("cargo");
        output
            .current_dir(&self.config.bench.cargo_dir)
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
            let stderr = t!(std::str::from_utf8(&output.stderr));
            let stdout = t!(std::str::from_utf8(&output.stdout));

            println!(
                "Unable to run {}\n\nSTDERR:\n{}\n\nSTDOUT:\n{}\n",
                self.display(),
                stderr,
                stdout
            );
            panic!("Unable to run instance");
        }

        if !warmup {
            let stderr = t!(std::str::from_utf8(&output.stderr));

            let mut times: Vec<TimeData> = stderr
                .trim()
                .lines()
                .filter_map(|line| {
                    let line = line.trim();
                    if line.starts_with("time:") {
                        let parts: Vec<&str> = line.split_ascii_whitespace().collect();
                        let name = parts.last().unwrap().to_string();
                        Some(TimeData {
                            name,
                            before_rss: parts[3].to_string(),
                            after_rss: parts[5].to_string(),
                            time: str::parse(parts[1].trim_end_matches(";")).unwrap(),
                        })
                    } else {
                        None
                    }
                })
                .collect();

            let totals: Vec<_> = times
                .iter()
                .enumerate()
                .filter(|(_, time)| time.name == "total")
                .collect();

            if totals.len() > 1 {
                let split_at = totals[totals.len() - 2].0 + 1;
                times = times.split_off(split_at);
            }

            let mut seen = HashSet::new();

            for time in &times {
                if !seen.insert(time.name.clone()) {
                    panic!(
                        "Duplicate -Z time entry for `{}` in {}",
                        time.name,
                        self.display()
                    );
                }
            }

            println!("Ran {} in {:?}", self.display(), duration);

            self.time.push(duration.as_secs_f64());
            self.times.push(times);
        }
    }

    fn summary_time(&self) -> f64 {
        self.time.iter().map(|t| *t).sum::<f64>() / self.time.len() as f64
    }

    fn result(&self) -> ResultConfig {
        ResultConfig {
            build: self.build.clone(),
            time: self.time.clone(),
            times: self.times.clone(),
        }
    }
}

pub fn bench(state: Arc<State>, matches: &ArgMatches) {
    let iterations =
        value_t!(matches, "iterations", usize).unwrap_or(state.config.iterations.unwrap_or(8));
    let iterations = std::cmp::max(1, iterations);
    println!("Using {} iterations", iterations);

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

    let mut modes = Vec::new();

    if matches.is_present("check") {
        modes.push(BenchMode::Check);
    }

    if matches.is_present("release") {
        modes.push(BenchMode::Release);
    }

    if matches.is_present("debug") {
        modes.push(BenchMode::Debug);
    }

    if modes.is_empty() {
        modes = vec![BenchMode::Check, BenchMode::Release, BenchMode::Debug];
    }

    let bench_configs: Vec<Config> = benchs
        .iter()
        .cloned()
        .flat_map(|bench| {
            modes.iter().map(move |&mode| Config {
                incremental: false,
                mode,
                bench: bench.clone(),
            })
        })
        .collect();

    t!(fs::create_dir_all(state.root.join("tmp")));
    let session_dir = crate::temp_dir(&state.root.join("tmp"));

    let session_dir2 = session_dir.clone();
    let state2 = state.clone();
    let _drop_session_dir = OnDrop(move || {
        crate::remove_recursively(&session_dir2);
        fs::remove_dir(state2.root.join("tmp")).ok();
    });

    let bench_configs_desc = bench_configs
        .iter()
        .map(|bench| bench.display())
        .collect::<Vec<_>>()
        .join(", ");

    println!("Benchmarks: {}", bench_configs_desc);

    let mut configs: Vec<ConfigInstances> = bench_configs
        .iter()
        .map(|config| ConfigInstances {
            config: config.clone(),
            builds: builds
                .iter()
                .map(|build| Instance {
                    time: Vec::new(),
                    times: Vec::new(),
                    session_dir: session_dir.clone(),
                    state: state.clone(),
                    build: build.name.clone(),
                    config: config.clone(),
                })
                .collect(),
        })
        .collect();

    configs.par_iter().for_each(|config| {
        config.builds.par_iter().for_each(|instance| {
            instance.prepare();
        });
    });

    for config in &mut configs {
        for instance in &mut *config.builds {
            println!("Warming up {}", instance.display());
            instance.run(true);
        }
        for _ in 0..iterations {
            for instance in &mut *config.builds {
                println!("Benching {}", instance.display());
                instance.run(false);
            }
        }
    }

    {
        let mut table = ascii_table::AsciiTable::default();

        let mut column = ascii_table::Column::default();
        column.header = "Benchmark".into();
        table.columns.insert(0, column);

        for (i, build) in builds.iter().enumerate() {
            let mut column = ascii_table::Column::default();
            column.header = build.name.clone();
            table.columns.insert(1 + i, column);
        }

        let rows: Vec<_> = configs
            .iter()
            .map(|config| {
                let mut row: Vec<String> = config
                    .builds
                    .iter()
                    .map(|build| format!("{:.06}", build.summary_time()))
                    .collect();
                row.insert(0, config.config.bench.name.clone());
                row
            })
            .collect();

        table.print(rows);
    }

    let build_names = builds
        .iter()
        .map(|build| build.name.as_str())
        .collect::<Vec<_>>()
        .join("__vs._");

    let time = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    let path = state
        .root
        .join("reports")
        .join(format!("{}_{}.html", build_names, time));

    t!(fs::create_dir_all(path.parent().unwrap()));

    let mut file = t!(File::create(&path));

    let result = Result {
        benchs: configs
            .iter()
            .map(|config| ResultBench {
                name: config.config.bench.name.clone(),
                builds: config
                    .builds
                    .iter()
                    .map(|instance| instance.result())
                    .collect(),
            })
            .collect(),
    };

    let title = builds
        .iter()
        .map(|build| build.name.as_str())
        .collect::<Vec<_>>()
        .join(" vs. ");

    let result = serde_json::to_string(&result).unwrap();

    let mut report = r#"<!doctype html>
    <html>
    <head>
      <title>Benchmark result for "#
        .to_string();

    report.push_str(&title);

    report.push_str(
        r#"</title>
      <link rel="stylesheet" href="../misc/report_style.css">
    <script>const DATA = "#,
    );

    report.push_str(&result);
    report.push_str(
        r#";</script>
    <script defer src="../misc/report_script.js"></script>
    </head>
    <body>
    </body>
    </html>"#,
    );

    t!(file.write_all(report.as_bytes()));

    println!("Extended report at {}", path.display());
}
