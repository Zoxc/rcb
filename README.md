# rustc benchmarking tool

This is a tool which is designed measure the time `rustc` takes to compile crates and manage locally built `rustc` branches.

The `fetch` command will extract a stage 1 compiler from a [Rust repo](https://github.com/rust-lang/rust). It will hash it and generate a build name which includes the repo name and Git branch with the hash used as a disambiguator. It will also fetch `config.toml` and Git information about the current commit and the upstream `bors` commit. It will fail if a build with the same hash exists. This makes it easy to compare various changes and branches with managing many Rust repositores.

It can measure the crates in the `benchs` directory with various `cargo` configuration. This is done with the `bench` command. It takes a list of builds and will build each crate a number of iterations and present the average result. The dependencies of the crate are not measured. Each crate is built with each build in turn to minimize noise due to performance drift of the system. The command presents a life summary of the runtimes and finally stores a more detailed report in the `reports` folder. [Here is an example](https://zoxc.github.io/rcb/reports/demo.html) of such a report. It includes information about passes and memory usage. It also has information about the difference of the builds (like build size) and will highlight `config.toml` differences and warn you if one of the builds is not against an upstream Rust commit.

## Setup

Build and copy the final binary to the repository root.

```sh
cd tool
cargo build --release
cp target/release/rcb ..
```

Create `rcb.toml` and edit it to let it know where your rustc repositories are.
```sh
cp rcb.example.toml rcb.toml
```

## Usage

First you build a compiler in one of your repositories, then you can fetch it to the `builds` folder in the repository root by this command:
```sh
rcb fetch <repo-name>
```
This will give you an identifier for the build, like `a~master~1`.

Once you have multiple builds you can compare them with the `bench` command:
```sh
rcb bench <builds..>
```

For example `rcb bench a~master~1 b~foo~1 --bench regex --check` would compare the `a~master~1` build versus the `b~foo~1` using the benchmark `regex` with `cargo check`.

Using the `bench` command will produce an HTML report in the `reports` folder in the repository root.

To get an idea about the noisy on your system you can specify the same build twice like `rcb bench a~master~1 a~master~1`. You can also do `rcb bench a~master~1 a~master~1 b~foo~1` to get an idea of noise while comparing.

## Command line options for `bench`

- `-n <iterations>`: The number of iterations to build crates for each build.
- `-j <jobs>`: The number of parallel instances for benchmarks, by default only 1 job runs at a time.
- `--details <mode>`: Pass `none` to disable collection of pass and memory details from `rustc` using `-Z time-precise` and `time` to enable it. By default it is enabled.

You can specifiy multiple types of builds and benchmarks additively. If some dimention is left unspecified, a default will be used.

- `-b <bench>` or `--bench <bench>`: This adds the benchmark `<bench>` from the `benchs` folder.
***
- `--check`: Adds `cargo check` builds.
- `--debug`: Adds `cargo build` builds.
- `--release`: Adds `cargo build --release` builds.
***
- `--incr-initial`: Adds the initial build for `rustc`'s' incremental compilation only.
- `--incr-none`: Adds a configuration without `rustc`'s' incremental compilation.
- `--incr-unchanged`: Adds a configuration with `rustc`'s' incremental compilation with a generated incremental cache and without any source changes.

There's also a number of per-build options. These all take a list of builds in the form of indices to the command line position separated by comma. You can also pass `a` to indicate all builds.

- `--env <build-list>:<arg>`: An enviroment variable which will be set when invoking `cargo`.
- `--cflag <build-list>:<arg>`: An argument which will be passed to `cargo`.
- `--rflag <build-list>:<arg>`: An argument which will be passed to `rustc`.
- `--threads <build-list>`: Avoids passing `-j 1` to `cargo` for the specified builds allowing parallelism within a crate compilation.
