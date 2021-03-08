# rustc benchmarking tool

## Setup

Build and copy the final binary to the repository root.

```sh
cargo build --release
cp target/release/rcb .
```

Create `rcb.toml` and edit it to let it know where your rustc repositories are.
```sh
cp rcb.example.toml rcb.toml
```

## Usage

First you build a compiler in one of your repositories, then you can fetch it to the `builds` folder in the repository root by this command:
```sh
rcp fetch <repo-name>
```
This will give you an identifier for the build, like `a~master~1`.

Once you have multiple builds you can compare them with the `bench` command:
```sh
rcp bench <builds..>
```

For example `rcb bench a~master~1 b~foo~1 --bench regex` would compare the `a~master~1` build versus the `b~foo~1` using the benchmark `regex`.

Using the `bench` command will produce an HTML report in the `reports` folder in the repository root.

To get an idea about the noisy on your system you can specify the same build twice like `rcb bench a~master~1 a~master~1`. You can also do `rcb bench a~master~1 a~master~1 b~foo~1` to get an idea of noise while comparing.