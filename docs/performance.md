# Performance

Build the release executable with `cargo build --release --locked`. The release
profile uses size optimization, LTO, one code generation unit, and stripped
symbols. It keeps Rust's default panic unwinding behavior.

Picker preparation checks for executable dependencies on PATH without starting
version commands. It lists tmux sessions and Docker containers concurrently with
the directory scan, then displays sessions, containers, and sorted directories
in that order. A slow Docker response still delays the picker; disable
`sources.docker` if containers are not needed.

The directory scan uses an explicit work list, stops at the configured depth,
and prunes ignored directories. It follows directory symlinks but does not
descend through links back to an ancestor. Repeated directory paths appear once.
Ignore matching reuses normalized patterns and compares path slices without
building temporary lists of prefixes and suffixes.

Picker labels are stored once. Input to fzf uses buffered writes and NUL
separators so filenames containing newlines or trailing spaces survive selection.
Cancellation and no matches return normally; other fzf failures report an error.

## Reproduce the measurement

Save a release binary before making changes, build the updated binary, and run:

```sh
python3 scripts/benchmark.py /path/to/before target/release/tmuxxer --runs 30
```

The script creates 10,000 empty project directories and temporary configuration.
It runs three warmups before collecting elapsed times. External commands are
stubs: tmux waits 20 ms, Docker waits 40 ms, and fzf drains input and cancels.
The directories scenario disables the tmux and Docker sources. The mixed
scenario enables both. Temporary files are removed afterward.

This measures process startup, source collection, and input transfer. It does
not measure interactive fzf rendering, real Docker latency, or cold filesystem
caches. Run it on the same machine with both binaries for a useful comparison.

Local results on x86_64 Linux with rustc 1.95.0, 30 measured runs:

| Measurement | Before | After |
| --- | ---: | ---: |
| Release executable | 1,021,816 bytes | 840,376 bytes |
| Directories, median | 30.89 ms | 20.08 ms |
| Mixed sources, median | 102.85 ms | 58.97 ms |

The executable is 17.8% smaller. Median preparation time fell 35.0% for directories
and 42.7% for mixed sources in this fixture. No dependencies were added.

## Dependency refresh

The subsequent dependency update uses serde 1.0.229, serde_json 1.0.151,
thiserror 2.0.20, and toml 1.1.5. Proptest remains at 1.11.0.
The lockfile uses current compatible transitive versions. Wasip2 stays at
1.0.1 because newer releases require Rust 1.87; it is an indirect test dependency
for WASI and is not part of the Linux release executable.

Using the same compiler and fixture, another 30-run comparison produced:

| Measurement | Before dependency refresh | After dependency refresh |
| --- | ---: | ---: |
| Release executable | 840,376 bytes | 828,768 bytes |
| Directories, median | 19.80 ms | 20.06 ms |
| Mixed sources, median | 58.68 ms | 59.00 ms |

The binary shrank by another 11,608 bytes. These timings do not demonstrate a
speed improvement from updating dependencies. All 93 tests pass on Rust 1.85.0
and 1.95.0. CI checks the minimum Rust version, and Dependabot is configured to
propose weekly Cargo and GitHub Actions updates.
