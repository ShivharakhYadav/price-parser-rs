# Benchmark methodology

What is measured, how, and — more usefully — what these numbers cannot tell you.

Reproduce with:

```sh
python tools/bench.py --upstream ../price-parser
```

Raw output: [`results.json`](results.json).

---

## What is compared

Three implementations, because reporting only the fastest would flatter the port:

| | |
|---|---|
| **upstream Python** | the baseline, run from a checkout at the pinned revision |
| **this port, from Python** | what a Python user actually gets, FFI overhead included |
| **this port, native Rust** | what a Rust caller gets |

The middle row is the honest comparison for anyone swapping this in as a drop-in replacement. The
last is the library's own speed.

## The corpus

Every price string in the frozen upstream suite — **1,178 of them**, extracted from
`tests/original/test_price_parsing.py` with `ast`.

Real scraped text, not inputs chosen to look fast. It includes the awkward cases: percentages, prices
with no currency, junk with no number, mixed separators, non-breaking spaces.

## Throughput

Best of **5 rounds**, each making **60 passes** over the corpus, after a warmup pass.

**Best rather than mean.** The fastest observed run is the least polluted by scheduling noise, and
taking the best on *both* sides keeps the bias pointing the same direction rather than flattering one.

**Rounds are long deliberately.** A single pass takes a few milliseconds, which is far too short to
time reliably — the first version of this benchmark used one pass and produced a table where the FFI
path appeared *faster* than the native call it wraps, which is impossible.

## Latency percentiles

Timed **per parse**, in a separate pass, ~23,000 samples per implementation. Reported as p50, p90,
p99, p99.9 and max.

A mean hides the tail, and the tail is what a caller notices.

**The caveat that matters:** timer overhead is tens of nanoseconds against a parse of a few
microseconds. That is visible in these numbers, on the order of a percent — it does not dominate, but
a p99 quoted without saying so would be overclaiming. Both sides pay it.

## Startup

Best of 5, timing a process that launches, parses one price, and exits. Measures everything paid
before the first useful answer: process launch, interpreter or runtime init, imports, lazy table
construction, regex compilation.

**The Rust side times the compiled binary directly, never `cargo run`.** The first version used
`cargo run` and reported ~700 ms — almost entirely cargo's freshness check — which would have made
the port look five times *slower* to start than CPython.

## Peak RSS — not measured on Windows

`results.json` reports `null` for peak memory on Windows. That is deliberate.

Two approaches were tried and both returned a constant ~3.4 MiB regardless of workload. That was not
taken on trust: a child process was made to allocate 50 MB and then 200 MB, and **the reported figure
did not move**. Declaring the `ctypes` signatures properly — `OpenProcess` returns a 64-bit `HANDLE`
that `ctypes` otherwise truncates to `c_int` — was necessary but not sufficient.

Rather than publish a number known to be wrong, the platform reports nothing.

**Linux got it wrong too, in a different way, before it got it right.** The first Linux
implementation used `getrusage(RUSAGE_CHILDREN).ru_maxrss` — which is a high-water mark across
*every child the process has ever reaped*. Once an earlier `cargo build` had run, it returned that
build's footprint forever: an identical 393 MiB for all three implementations. It reads as a real
measurement and is nothing of the kind.

It now samples `/proc/<pid>/status` `VmHWM` while the child is alive, which is per-process and cannot
be polluted that way. **CI runs this benchmark on Linux on every push**, so the one number the
committed Windows results cannot supply is measured where it can be trusted, and is visible in the
job log.

The pattern is worth naming: **both wrong versions returned a constant**, and a constant is what a
broken measurement looks like. Neither would have been caught by a test — only by asking whether the
number could possibly be true.

Neither implementation reads its own RSS from inside: doing that from Rust would mean an unsafe FFI
call, and the zero-`unsafe` guarantee is worth more than a memory number.

## Build profile

The extension module reports its own profile as `price_parser.__build__`, and the benchmark
**refuses to run against a debug build**.

`maturin develop` defaults to debug, which is roughly twenty times slower. The very first run of this
benchmark compared a debug extension against a release native binary and concluded the port was five
times *slower* than the Python it replaces.

## How much to trust the numbers

Less than their precision suggests.

The Python baseline is steady to about ±5% across runs. The Rust figures are noisier in relative
terms simply because they are smaller: across repeated samples the native throughput swung between
roughly 2.3× and 6.1× the baseline. **The two Rust paths cannot be told apart on this hardware** —
the FFI overhead is real but smaller than the machine's jitter.

The honest summary is **around 4× on throughput and around 10× on startup**, not precise multiples.
Anyone wanting firm figures should re-run on a quiet machine; the command is at the top of this file.
