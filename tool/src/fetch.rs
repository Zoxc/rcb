use crate::temp_dir;
use crate::Build;
use crate::OnDrop;
use crate::State;
use clap::ArgMatches;
use data_encoding::HEXLOWER;
use rayon::prelude::*;
use ring::digest::{Context, SHA256};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::{convert::TryFrom, sync::Arc};
use std::{ffi::OsStr, io, path::PathBuf, process::Command};
use std::{fs, path::Path};

const TRIPLE: &str = env!("TARGET");

/// Copies a file from `src` to `dst`
pub fn copy(state: &State, src: &Path, dst: &Path) {
    if src == dst {
        return;
    }
    let metadata = t!(src.symlink_metadata());
    if metadata.file_type().is_symlink() {
        let link = t!(fs::read_link(src));
        if state.verbose {
            println!("Skipping {} linking to {}", src.display(), link.display());
        }
        return;
    }
    if let Err(e) = fs::copy(src, dst) {
        panic!(
            "failed to copy `{}` to `{}`: {}",
            src.display(),
            dst.display(),
            e
        )
    }
}

/// Copies the `src` directory recursively to `dst`.
pub fn copy_recursively(state: &State, src: &Path, dst: &Path) {
    t!(fs::create_dir_all(&dst));
    for f in t!(fs::read_dir(src)) {
        let f = t!(f);
        let path = f.path();
        let name = path.file_name().unwrap();
        let dst = dst.join(name);
        if t!(f.file_type()).is_dir() {
            t!(fs::create_dir(&dst));
            copy_recursively(state, &path, &dst);
        } else {
            let _ = fs::remove_file(&dst);
            copy(state, &path, &dst);
        }
    }
}

pub fn list_files(root: &Path, relative: &Path, out: &mut Vec<String>) {
    for f in t!(fs::read_dir(root.join(relative))) {
        let f = t!(f);
        let path = f.path();
        let name = path.file_name().unwrap();
        let rel = relative.join(name);
        if t!(f.file_type()).is_dir() {
            list_files(root, &rel, out);
        } else {
            out.push(rel.to_string_lossy().into_owned());
        }
    }
}

fn capture<I, S>(cmd: &str, args: I, path: &Path) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(cmd)
        .current_dir(path)
        .args(args)
        .output()
        .expect("failed to execute process");

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        None
    }
}

fn sha256_digest<R: Read>(mut reader: R, context: &mut Context) -> io::Result<()> {
    let mut buffer = [0; 1024];

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        context.update(&buffer[..count]);
    }

    Ok(())
}

fn sha256_from_file(path: &Path, context: &mut Context) -> io::Result<()> {
    let input = File::open(path)?;
    context.update(&input.metadata()?.len().to_le_bytes());
    let reader = BufReader::new(input);
    sha256_digest(reader, context)
}

fn get_build_signature(dir: &Path) -> (String, u64) {
    let mut files = Vec::new();

    list_files(&dir, &Path::new(""), &mut files);

    files.sort();

    let size = files
        .iter()
        .map(|file| t!(dir.join(file).metadata()).len())
        .sum();

    let digests: Vec<_> = files
        .par_iter()
        .map(|file| {
            let mut context = Context::new(&SHA256);
            context.update(&u64::try_from(file.len()).unwrap().to_le_bytes());
            context.update(file.as_bytes());
            t!(sha256_from_file(&dir.join(file), &mut context));
            context.finish()
        })
        .collect();

    let mut context = Context::new(&SHA256);
    context.update(&u64::try_from(files.len()).unwrap().to_le_bytes());

    for digest in digests {
        context.update(digest.as_ref());
    }

    let signature = context.finish();

    (HEXLOWER.encode(signature.as_ref()), size)
}

fn find_build_name(state: &State, prefix: &str, signature: &str) -> (String, PathBuf) {
    let mut i = 1;
    loop {
        let candidate = format!("{}~{}", prefix, &signature[0..i]);

        let candidate_path = state.root.join("builds").join(&candidate);

        if !candidate_path.exists() {
            return (candidate, candidate_path);
        } else {
            let build = t!(fs::read_to_string(candidate_path.join("build.toml")));
            let build: Build = t!(toml::from_str(&build));
            if build.signature == signature {
                panic!("Build already exists as {}", candidate);
            }
        }

        i += 1;

        if i > signature.len() {
            panic!("Unable to find a unique name for the build");
        }
    }
}

pub fn fetch(state: Arc<State>, matches: &ArgMatches) {
    t!(fs::create_dir_all(state.root.join("builds")));

    let repo = matches
        .value_of("REPO")
        .map(|repo| state.repo(repo.to_owned()))
        .unwrap_or_else(|| state.default_repo());

    let repo_path = state.repo_path(&repo);

    println!(
        "Fetching stage1 build from {} at {}, {}",
        repo,
        repo_path.display(),
        env!("TARGET")
    );

    let stage1 = state
        .repo_path(&repo)
        .join("build")
        .join(TRIPLE)
        .join("stage1")
        .to_owned();

    let mut rustc = stage1.join("bin").join("rustc").to_owned();
    rustc.set_extension(std::env::consts::EXE_EXTENSION);

    println!("exe {}", rustc.display(),);

    let branch = capture(
        "git",
        &["symbolic-ref", "--short", "-q", "HEAD"],
        &repo_path,
    );
    let commit = capture("git", &["rev-parse", "--short", "-q", "HEAD"], &repo_path);

    match (&branch, &commit) {
        (Some(branch), Some(commit)) => println!("From git branch {} on commit {}", branch, commit),
        _ => (),
    }

    let tmp_path = temp_dir(&state.root.join("builds"));

    let tmp_path2 = tmp_path.clone();
    let _drop_tmp_dir = OnDrop(move || {
        crate::remove_recursively(&tmp_path2);
    });

    copy_recursively(&state, &stage1, &tmp_path.join("stage1"));

    let (signature, build_size) = get_build_signature(&tmp_path);

    let (name, build_path) = find_build_name(
        &state,
        &format!("{}~{}", repo, branch.as_deref().unwrap_or("")),
        &signature,
    );

    t!(fs::rename(&tmp_path, &build_path));

    {
        let build = Build {
            name: name.clone(),
            repo,
            repo_path: repo_path.clone(),
            branch,
            commit,
            size: build_size,
            size_display: kib::format(build_size),
            signature,
            triple: TRIPLE.to_owned(),
        };
        let mut file = t!(File::create(build_path.join("build.toml")));
        t!(file.write_all(toml::to_string_pretty(&build).unwrap().as_bytes()));
    }

    {
        let build_config = t!(fs::read_to_string(repo_path.join("config.toml")));
        let build_config: toml::Value = toml::from_str(&build_config).expect("Invalid config.toml");
        let mut file = t!(File::create(build_path.join("config.toml")));
        t!(file.write_all(toml::to_string_pretty(&build_config).unwrap().as_bytes()));
    }

    println!("Build {} ({})", name, kib::format(build_size));

    if !rustc.exists() {
        panic!("Could not find build executable at `{}`", rustc.display());
    }
}
