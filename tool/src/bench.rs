use crate::Build;
use crate::State;
use clap::ArgMatches;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::fs;

struct BenchConfig {
    incremental: bool,
    mode: BenchMode,
}

enum BenchMode {
    Check,
    Debug,
    Release,
}

pub fn bench(state: State, matches: &ArgMatches) {
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
                Some(path)
            } else {
                None
            }
        })
        .collect();

    let session_dir = crate::temp_dir(&state.root.join("tmp"));

    t!(fs::create_dir_all(session_dir));

    builds
        .par_iter()
        .for_each(|build| t!(fs::create_dir_all("path")))
}
