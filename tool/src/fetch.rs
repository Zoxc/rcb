use crate::State;
use clap::ArgMatches;

pub fn fetch(state: State, matches: &ArgMatches) {
    let repo = matches
        .value_of("REPO")
        .map(|repo| state.repo(repo.to_owned()))
        .unwrap_or_else(|| state.default_repo());

    println!(
        "Fetching stage1 build from {} at {}, {}",
        repo,
        state.repo_path(&repo).display(),
        env!("TARGET")
    );
}
