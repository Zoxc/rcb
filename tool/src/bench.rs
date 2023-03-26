use crate::bench::display::Display;
use crate::term;
use crate::term::View;
use crate::term::Viewable;
use crate::Build;
use crate::OnDrop;
use crate::State;
use clap::value_t;
use clap::ArgMatches;
use core::panic;
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};
use serde_derive::{Deserialize, Serialize};
use std::cmp;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::Path,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread::sleep,
    time::{Duration, Instant},
};

mod display;

#[derive(Serialize, Default)]
struct BuildConfig {
    index: usize,
    name: String,
    threads: bool,
    rflags: Vec<String>,
    cflags: Vec<String>,
    envs: Vec<(String, String)>,
}

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
    details: bool,
    incremental: IncrementalMode,
    mode: BenchMode,
    bench: Arc<Bench>,
}

impl Config {
    fn view(&self, view: &mut View) {
        view!(
            view,
            term::default_color(),
            //term::bold(),
            self.bench.name,
            term::default_color()
        );
        ":".view(view);
        match self.mode {
            BenchMode::Check => view!(
                view,
                term::color(137, 114, 186),
                "check",
                term::default_color()
            ),
            BenchMode::Debug => view!(
                view,
                term::color(204, 174, 75),
                "debug",
                term::default_color()
            ),
            BenchMode::Release => view!(
                view,
                term::color(102, 166, 209),
                "release",
                term::default_color()
            ),
        }

        match self.incremental {
            IncrementalMode::Initial => view!(
                view,
                ":",
                term::color(132, 143, 99),
                "initial",
                term::default_color()
            ),
            IncrementalMode::Unchanged => view!(
                view,
                ":",
                term::color(132, 143, 99),
                "unchanged",
                term::default_color()
            ),
            IncrementalMode::None => (),
        }
    }

    fn display(&self) -> String {
        let start = format!("{}:{}", self.bench.name, self.mode.display());
        match self.incremental {
            IncrementalMode::Initial => format!("{}:initial", start),
            IncrementalMode::Unchanged => format!("{}:unchanged", start),
            IncrementalMode::None => start,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum IncrementalMode {
    None,
    Initial,
    Unchanged,
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

fn crate_matches(path: &Path, krate: &str) -> Vec<(String, PathBuf)> {
    t!(fs::read_dir(path))
        .filter_map(|f| {
            let f = t!(f);
            let path = f.path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if let Some(i) = name.as_bytes().iter().rposition(|c| *c == '-' as u8) {
                if &name[0..i] == krate {
                    return Some((name, path));
                }
            }
            None
        })
        .collect()
}

fn remove_fingerprint(mut fingerprints: Vec<(String, PathBuf)>, krate: &str) {
    if fingerprints.len() != 1 {
        panic!(
            "Didn't find exactly one fingerprint for {}, found {:#?}",
            krate, fingerprints
        );
    }

    crate::remove_recursively(&fingerprints.pop().unwrap().1);
}

#[derive(Serialize, Clone)]
struct TimeData {
    name: String,
    time: f64,
    before_rss: u64,
    after_rss: u64,
}

#[derive(Serialize)]
struct ResultConfig {
    build: String,
    time: Vec<f64>,
    times: Option<Vec<Vec<TimeData>>>,
}

#[derive(Serialize)]
struct ResultBench {
    name: String,
    builds: Vec<ResultConfig>,
}

#[derive(Serialize)]
struct Result {
    builds: Vec<Build>,
    build_configs: Vec<Arc<BuildConfig>>,
    benchs: Vec<ResultBench>,
}

struct Instance {
    config_index: usize,
    build_index: usize,
    session_dir: PathBuf,
    state: Arc<State>,
    build: Arc<BuildConfig>,
    config: Config,
    time: Vec<f64>,
    times: Vec<Vec<TimeData>>,
}

pub(crate) struct ConfigInstances {
    config_index: usize,
    config: Config,
    builds: Vec<Instance>,
}

impl Instance {
    fn display(&self) -> String {
        format!(
            "benchmark {} with {}",
            self.config.display(),
            self.build.name
        )
    }

    fn path(&self) -> PathBuf {
        self.session_dir.join(format!(
            "{}-{}",
            self.build.index,
            &self.config.display().replace(":", "$")
        ))
    }

    fn cargo(&self, prepare: bool) -> Command {
        let mut output = Command::new("cargo");
        output
            .current_dir(&self.config.bench.cargo_dir)
            .stdin(Stdio::null())
            .env(
                "RUSTC",
                self.state
                    .root
                    .join("builds")
                    .join(&self.build.name)
                    .join("stage1")
                    .join("bin")
                    .join("rustc"),
            )
            .env(
                "CARGO_INCREMENTAL",
                if self.config.incremental != IncrementalMode::None {
                    "1"
                } else {
                    "0"
                },
            )
            .env("CARGO_TARGET_DIR", self.path());

        if !prepare {
            output
                .env("RUSTC_WRAPPER", &self.state.exe)
                .env("RCB_ACT_AS_RUSTC", "1");
        }

        match self.config.mode {
            BenchMode::Check => {
                output.arg("check");
            }
            BenchMode::Debug => {
                output.arg("build");
            }
            BenchMode::Release => {
                output.arg("build");
                output.arg("--release");
            }
        }

        let mut rflags = if self.config.details {
            vec![
                "-Ztime-passes".to_owned(),
                "-Ztime-passes-format=json".to_owned(),
            ]
        } else {
            Vec::new()
        };
        rflags.extend_from_slice(&self.build.rflags);
        output.env("RUSTFLAGS", rflags.join(" "));

        if !prepare && !self.build.threads {
            output.arg("-j1");
        }
        for cflag in &self.build.cflags {
            output.arg(cflag);
        }
        for (env, val) in &self.build.envs {
            output.env(env, val);
        }

        output
    }

    fn prepare(&mut self) {
        t!(fs::create_dir_all(self.path()));

        let mut output = self.cargo(true);
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

        // Run an extra time to remove cached queries that can't follow from one unchanged
        // session to the next
        if self.config.incremental == IncrementalMode::Unchanged {
            self.run(true, true, None);
        }
    }

    fn remove_fingerprint(&self) {
        let target_profile = self.path().join(match self.config.mode {
            BenchMode::Check | BenchMode::Debug => "debug",
            BenchMode::Release => "release",
        });

        let krate = &self.config.bench.name;

        let build_scripts = crate_matches(&target_profile.join("build"), krate);

        let mut fingerprints = crate_matches(&target_profile.join(".fingerprint"), krate);

        // Remove build scripts which can share the name of the main crate
        fingerprints.retain(|(name, _)| {
            build_scripts
                .iter()
                .find(|(build, _)| name == build)
                .is_none()
        });

        remove_fingerprint(fingerprints, krate);

        if self.config.incremental == IncrementalMode::Initial {
            let incremental = crate_matches(&target_profile.join("incremental"), krate);

            remove_fingerprint(incremental, krate);
        }
    }

    fn run(&mut self, incremental_extra: bool, warmup: bool, display: Option<&Mutex<Display>>) {
        self.remove_fingerprint();

        let mut output = self.cargo(false);

        let output = t!(output.output());

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

        if incremental_extra {
            return;
        }

        if warmup {
            display.map(|display| {
                display
                    .lock()
                    .unwrap()
                    .report_warmup(self.config_index, self.build_index)
            });
        } else {
            let stderr = t!(std::str::from_utf8(&output.stderr));

            let mut time: Vec<_> = stderr
                .trim()
                .lines()
                .filter_map(|line| {
                    let line = line.trim();
                    if line.starts_with("rcb-rustc-timer:") {
                        let parts: Vec<&str> = line.split(":").collect();
                        let time = parts.last().unwrap();
                        let time = Duration::from_micros(str::parse(time).unwrap());
                        Some(time.as_secs_f64())
                    } else {
                        None
                    }
                })
                .collect();

            if time.len() != 1 {
                panic!(
                    "Multiple time results for {}\nSTDERR:{}",
                    self.display(),
                    stderr
                );
            }
            let time = time.pop().unwrap();

            display.map(|display| {
                display
                    .lock()
                    .unwrap()
                    .report(self.config_index, self.build_index, time)
            });

            //println!("Ran {} in {:.04}s", self.display(), time);

            self.time.push(time);

            if self.config.details {
                let mut times: Vec<TimeData> = stderr
                    .trim()
                    .lines()
                    .filter_map(|line| {
                        let line = line.trim();
                        if line.starts_with("time:") {
                            let json: serde_json::Value = serde_json::from_str(&line[5..]).unwrap();
                            Some(TimeData {
                                name: json["pass"].as_str().unwrap().to_owned(),
                                before_rss: json["rss_start"].as_u64().unwrap(),
                                after_rss: json["rss_end"].as_u64().unwrap(),
                                time: json["time"].as_f64().unwrap(),
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

                let mut data: HashMap<String, TimeData> = HashMap::new();

                for time in &times {
                    data.entry(time.name.clone())
                        .and_modify(|prev| {
                            prev.time += time.time;
                            prev.before_rss = cmp::min(prev.before_rss, time.before_rss);
                            prev.after_rss = cmp::max(prev.after_rss, time.after_rss);
                        })
                        .or_insert(time.clone());
                }

                let times: Vec<_> = times.iter().map(|time| data[&time.name].clone()).collect();

                self.times.push(times);
            }
        }
    }

    fn result(&self) -> ResultConfig {
        ResultConfig {
            build: self.build.name.clone(),
            time: self.time.clone(),
            times: if self.config.details {
                Some(self.times.clone())
            } else {
                None
            },
        }
    }
}

fn build_opt(
    matches: &ArgMatches,
    name: &str,
    has_value: bool,
    builds: &mut [BuildConfig],
    mut apply: impl FnMut(&mut BuildConfig, &str),
) {
    if let Some(args) = matches.values_of(name) {
        for arg in args {
            let (arg, val) = if has_value {
                let i = arg
                    .find(":")
                    .unwrap_or_else(|| panic!("Argument `{}` to {} has no value", arg, name));
                let (k, v) = arg.split_at(i + 1);
                (k.strip_suffix(":").unwrap_or(k), v)
            } else {
                (arg, "")
            };
            for build in arg.split(",") {
                if build == "a" {
                    builds.iter_mut().for_each(|build| apply(build, val));
                } else {
                    let i = str::parse::<usize>(build)
                        .unwrap_or_else(|_| panic!("Expected `a` or build numer for opt {}", name));
                    let build = builds.get_mut(i - 1).unwrap_or_else(|| {
                        panic!("Build number {} out of bounds for opt {}", i, name)
                    });
                    apply(build, val);
                }
            }
        }
    }
}

fn build_configs(matches: &ArgMatches, builds: &[Build]) -> Vec<Arc<BuildConfig>> {
    let mut build_configs: Vec<_> = builds
        .iter()
        .enumerate()
        .map(|(index, build)| BuildConfig {
            index,
            name: build.name.clone(),
            ..Default::default()
        })
        .collect();

    build_opt(
        matches,
        "threads",
        false,
        &mut build_configs,
        |config, _| {
            config.threads = true;
        },
    );

    build_opt(matches, "rflag", true, &mut build_configs, |config, val| {
        config.rflags.push(val.to_owned());
    });

    build_opt(matches, "cflag", true, &mut build_configs, |config, val| {
        config.cflags.push(val.to_owned());
    });

    build_opt(matches, "env", true, &mut build_configs, |config, val| {
        let i = val
            .find("=")
            .unwrap_or_else(|| panic!("Enviroment variable `{}` has no value", val));
        let (k, v) = val.split_at(i + 1);
        if k.len() < 2 {
            panic!("Enviroment variable `{}` has no key", val);
        }
        config
            .envs
            .push((k[0..(k.len() - 1)].to_owned(), v.to_owned()));
    });

    build_configs
        .into_iter()
        .map(|config| Arc::new(config))
        .collect()
}

fn run_benchs(
    state: &State,
    configs: &mut Vec<ConfigInstances>,
    iterations: usize,
    warmups: usize,
    matches: &ArgMatches,
    display: Arc<Mutex<Display>>,
) {
    let threads = std::cmp::max(value_t!(matches, "jobs", usize).unwrap_or(1), 1);

    if threads == 1 {
        for config in configs {
            run_bench(config, iterations, warmups, 0, None, &display);
        }
    } else {
        let costs = state.root.join("benchs").join("cost.toml");
        let costs = t!(fs::read_to_string(costs));
        let costs: HashMap<String, f64> = t!(toml::from_str(&costs));
        let mut configs: Vec<_> = configs
            .iter_mut()
            .map(|config| {
                let cost = *costs.get(&config.config.display()).unwrap_or(&1.0);
                (config, cost)
            })
            .collect();
        configs.sort_by(|a, b| a.1.total_cmp(&b.1));
        let configs = Mutex::new(configs);

        let last_event: Mutex<Vec<_>> = Mutex::new((0..threads).map(|_| Instant::now()).collect());

        rayon::scope(|scope| {
            for i in 0..threads {
                let configs = &configs;
                let last_event = &last_event;
                let display = display.clone();
                scope.spawn(move |_| loop {
                    let i = i;
                    let config = configs.lock().unwrap().pop();

                    if let Some((config, _)) = config {
                        run_bench(config, iterations, warmups, i, Some(&last_event), &display);
                    } else {
                        break;
                    }
                })
            }
        });
    }
}

fn set_event(thread: usize, last_event: Option<&Mutex<Vec<Instant>>>) {
    last_event.map(|last_event| last_event.lock().unwrap()[thread] = Instant::now());
}

fn wait_event(thread: usize, last_event: Option<&Mutex<Vec<Instant>>>) {
    let last_event = if let Some(last_event) = last_event {
        last_event
    } else {
        return;
    };

    loop {
        {
            let now = Instant::now();
            let mut events = last_event.lock().unwrap();
            if events
                .iter()
                .enumerate()
                .filter(|&(i, _)| i != thread)
                .all(|(_, &time)| now.saturating_duration_since(time).as_millis() > 500)
            {
                events[thread] = Instant::now();
                return;
            }
        }
        sleep(Duration::from_millis(50));
    }
}

fn run_bench(
    config: &mut ConfigInstances,
    iterations: usize,
    warmups: usize,
    thread: usize,
    last_event: Option<&Mutex<Vec<Instant>>>,
    display: &Mutex<Display>,
) {
    display.lock().unwrap().start_config(config.config_index);

    for _ in 0..warmups {
        for instance in &mut *config.builds {
            wait_event(thread, last_event);
            instance.run(false, true, Some(display));
            set_event(thread, last_event);
        }
    }

    for _ in 0..iterations {
        for instance in &mut *config.builds {
            sleep(Duration::from_millis(200));
            wait_event(thread, last_event);
            instance.run(false, false, Some(display));
            set_event(thread, last_event);
        }
    }
}

pub fn bench(state: Arc<State>, matches: &ArgMatches) {
    let start = Instant::now();

    let details = matches
        .value_of("details")
        .map(|v| match v {
            "none" => false,
            "time" => true,
            _ => panic!("Unknown details value `{}`", v),
        })
        .unwrap_or(true);

    let iterations =
        value_t!(matches, "iterations", usize).unwrap_or(state.config.iterations.unwrap_or(8));
    let iterations = std::cmp::max(1, iterations);
    let warmups = value_t!(matches, "warmup", usize).unwrap_or(1);

    println!(
        "Using {} iterations with {} warmup round(s)",
        iterations, warmups
    );

    let builds: Vec<Build> = matches
        .values_of("BUILD")
        .unwrap()
        .map(|build_name| {
            let build_path = state.root.join("builds").join(build_name);
            let build = build_path.join("build.toml");
            if !build.exists() {
                panic!("Cannot find build `{}`", build_name);
            }
            let build = t!(fs::read_to_string(build));
            let build: Build = t!(toml::from_str(&build));
            build
        })
        .collect();

    let build_configs = build_configs(matches, &builds);

    println!("");
    for (i, (build, build_config)) in builds.iter().zip(build_configs.iter()).enumerate() {
        println!(
            "Build #{} {} ({} {})",
            i + 1,
            build.name,
            build.commit.as_deref().unwrap_or(""),
            kib::format(build.size),
        );
        if build_config.threads {
            println!("    Default thread count");
        }
        for rflag in &build_config.rflags {
            println!("    rustc:{}", rflag);
        }
        for cflag in &build_config.cflags {
            println!("    cargo:{}", cflag);
        }
        for (env, val) in &build_config.envs {
            println!("    env:{} = {}", env, val);
        }
        println!("");
    }

    let benchs: Vec<Arc<Bench>> = t!(fs::read_dir(state.root.join("benchs")))
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

    let selected_benchs: Vec<_> = matches
        .values_of("bench")
        .map(|b| b.collect())
        .unwrap_or_default();

    let benchs: Vec<Arc<Bench>> = if selected_benchs.is_empty() {
        benchs
            .iter()
            .cloned()
            .filter(|bench| state.config.benchs.contains(&bench.name))
            .collect()
    } else {
        selected_benchs
            .into_iter()
            .map(|bench| {
                benchs
                    .iter()
                    .find(|b| b.name == bench)
                    .unwrap_or_else(|| panic!("Unable to find bench `{}`", bench))
                    .clone()
            })
            .collect()
    };

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

    let mut incr_modes = Vec::new();

    if matches.is_present("incr-none") {
        incr_modes.push(IncrementalMode::None);
    }

    if matches.is_present("incr-initial") {
        incr_modes.push(IncrementalMode::Initial);
    }

    if matches.is_present("incr-unchanged") {
        incr_modes.push(IncrementalMode::Unchanged);
    }

    if incr_modes.is_empty() {
        incr_modes = vec![
            IncrementalMode::None,
            IncrementalMode::Initial,
            IncrementalMode::Unchanged,
        ];
    }

    let incr_modes = &incr_modes;
    let bench_configs: Vec<Config> = benchs
        .iter()
        .cloned()
        .flat_map(|bench| {
            modes.iter().flat_map(move |&mode| {
                let bench = bench.clone();
                incr_modes.iter().map(move |&incremental| Config {
                    details,
                    incremental,
                    mode,
                    bench: bench.clone(),
                })
            })
        })
        .collect();

    t!(fs::create_dir_all(state.root.join("tmp")));

    // Cleanup stale temporary directories
    fs::read_dir(state.root.join("tmp"))
        .map(|entries| {
            for entry in entries {
                entry
                    .map(|entry| {
                        if entry
                            .file_name()
                            .to_str()
                            .map_or(false, |f| f.starts_with("rcb-"))
                        {
                            crate::remove_recursively(&entry.path());
                        }
                    })
                    .ok();
            }
        })
        .ok();

    let session_dir = crate::temp_dir(&state.root.join("tmp"));

    let session_dir2 = session_dir.clone();
    let _drop_session_dir = OnDrop(move || {
        crate::remove_recursively(&session_dir2);
    });

    let bench_configs_desc = bench_configs
        .iter()
        .map(|bench| bench.display())
        .collect::<Vec<_>>()
        .join(", ");

    println!("Benchmarks: {}\n", bench_configs_desc);

    let mut configs: Vec<ConfigInstances> = bench_configs
        .iter()
        .enumerate()
        .map(|(config_index, config)| ConfigInstances {
            config_index,
            config: config.clone(),
            builds: build_configs
                .iter()
                .enumerate()
                .map(|(build_index, build)| Instance {
                    config_index,
                    build_index,
                    time: Vec::new(),
                    times: Vec::new(),
                    session_dir: session_dir.clone(),
                    state: state.clone(),
                    build: build.clone(),
                    config: config.clone(),
                })
                .collect(),
        })
        .collect();

    {
        let total: usize = configs.iter().map(|config| config.builds.len()).sum();
        let view = Mutex::new((View::new(), 0));

        let print = || {
            let mut lock = view.lock().unwrap();
            lock.0.rewind();
            view!(
                &mut lock.0,
                term::progress_bar(
                    &format!("Preparing benchmarks {}/{}: ", lock.1, total),
                    lock.1,
                    total
                )
            );
            lock.0.flush();
        };

        print();

        let start = Instant::now();

        configs.par_iter_mut().for_each(|config| {
            config.builds.par_iter_mut().for_each(|instance| {
                instance.prepare();
                view.lock().unwrap().1 += 1;
                print();
            });
        });

        view.into_inner().unwrap().0.rewind();

        let duration = start.elapsed();
        let seconds = duration.as_secs() % 60;
        let minutes = (duration.as_secs() / 60) % 60;
        let hours = minutes / 60;

        println!(
            "Prepared benchmarks in {:02}:{:02}:{:02}",
            hours, minutes, seconds
        );
    }

    {
        let display = Arc::new(Mutex::new(Display::new(&configs, iterations, warmups)));

        display.lock().unwrap().refresh();

        run_benchs(
            &state,
            &mut configs,
            iterations,
            warmups,
            matches,
            display.clone(),
        );

        display.lock().unwrap().complete();
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

    let title = builds
        .iter()
        .map(|build| build.name.as_str())
        .collect::<Vec<_>>()
        .join(" vs. ");

    let result = Result {
        builds,
        build_configs,
        benchs: configs
            .iter()
            .map(|config| ResultBench {
                name: config.config.display(),
                builds: config
                    .builds
                    .iter()
                    .map(|instance| instance.result())
                    .collect(),
            })
            .collect(),
    };

    let result = serde_json::to_string(&result).unwrap();

    let mut report = r#"<!doctype html>
    <html>
    <head>
      <meta charset="UTF-8">
      <title>Benchmark results for "#
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

    let duration = start.elapsed();
    let seconds = duration.as_secs() % 60;
    let minutes = (duration.as_secs() / 60) % 60;
    let hours = minutes / 60;

    println!("Completed in {:02}:{:02}:{:02}", hours, minutes, seconds);
}
