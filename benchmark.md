# Benchmark

Performance comparison between `eazyshell` and GNU `bash`, measured with the release build on Linux.

## Methodology

- **Binary**: `target/release/eazyshell` (release build)
- **Reference**: system `bash`
- **Iterations**: median of 5 runs per benchmark
- **Measurement**: wall-clock elapsed time for the full shell run (including process startup)
- All output redirected to `/dev/null`; only timing measured
- Benchmarks run in an isolated directory with the shell reading from a pipe (non-interactive)

## Results

| Benchmark | eazyshell | bash | ratio (eazyshell/bash) |
|---|---:|---:|---:|
| Startup (empty input) | 3.96 ms | 5.25 ms | 0.75x |
| 200 external commands | 7.21 ms | 10.37 ms | 0.70x |
| 500 builtin `echo` | 7.09 ms | 11.30 ms | 0.63x |
| 50-stage pipeline | 31.37 ms | 23.26 ms | 1.35x |
| 30 command substitutions | 110.85 ms | 59.52 ms | 1.86x |
| 1000 arithmetic expansions | 6.10 ms | 17.44 ms | 0.35x |

## Interpretation

`eazyshell` is **faster than bash** on the common paths:

- Shell startup (0.75x)
- External command execution (0.70x)
- Builtin execution (0.63x)
- Arithmetic expansion (0.35x) — notably ~3x faster

`eazyshell` is **slower than bash** on:

- **Multi-stage pipelines (1.35x)** — bash has highly optimized pipe internals.
- **Command substitution (1.86x)** — the main cost. Each `$(...)` spawns a fresh `eazyshell` process (process-per-substitution design), which is inherently heavier and compounds with multiple/inline substitutions.

## Notes

These are order-of-magnitude reference numbers, not micro-benchmarks. The pipeline and command-substitution gaps are expected from the current design (a subprocess shell per substitution) rather than a sign of pathological performance. Closing the substitution gap would require in-process substitution evaluation rather than spawning a new shell.
