# price-parser-rs

[![CI](https://github.com/ShivharakhYadav/price-parser-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/ShivharakhYadav/price-parser-rs/actions/workflows/ci.yml)

A Rust port of [`scrapinghub/price-parser`](https://github.com/scrapinghub/price-parser) — extract a
price amount and currency symbol from a raw text string.

> **Not affiliated with, nor endorsed by, Scrapinghub.** This is an independent port. The original
> Python implementation is copyright Scrapinghub and BSD-3-Clause licensed; see [LICENSE](LICENSE),
> retained verbatim as that licence requires.

Built for [Port Mortem — Code Resurrection 2026](https://coderesurrection.com/2026), Track D
(Python → Rust).

---

## The original test suite passes, unmodified

```
1059 passed, 134 xfailed
```

That is upstream's own test file, byte-for-byte, executing against Rust.

Not a rewrite of the tests. Not an adapter around them. Not assertions translated into Rust. Their
file, run unchanged, against this implementation — through a [PyO3](https://pyo3.rs) extension module
that presents the same import path and public API as the Python package, so their
`from price_parser import Price` resolves to this crate.

The 134 `xfailed` are upstream's own two `XFAIL` corpora, marked `strict=True`: cases upstream does
not pass either. They are expected to fail, and do.

## Why you can believe that

A claim like the one above is only worth the evidence behind it, so the tests are frozen and the
freezing is checked.

- Everything under [`tests/original/`](tests/original/) is vendored byte-for-byte from upstream at
  revision `64e213a`, recorded in [`PROVENANCE.md`](tests/original/PROVENANCE.md).
- Every file has a SHA-256 in a committed manifest. `python tools/verify_hashes.py` re-checks it and
  exits non-zero on any change, addition or removal.
- Git agrees independently: `git log -- tests/original/` shows **one commit ever** against that
  directory — the one that vendored it.
- `.gitattributes` marks the directory `-text`, so no line-ending conversion can alter those bytes on
  checkout and quietly invalidate the hashes.

[CI](.github/workflows/ci.yml) re-runs the whole chain on every push: hashes, git's view of the
vendored tests, Rust tests, lints under both feature configurations, generated-file freshness, the
extension build, both test suites, and a fixed-seed differential fuzz against upstream.

**The hash check runs first, deliberately** — and again at the end. A green build reporting the suite
passing while the suite had been edited would be worth nothing, so the evidence is established before
any claim is made.

## What made this port non-trivial

Three things resisted a direct translation. They are the reason this took real work.

### A lookahead Rust cannot express

Upstream identifies currency codes like `NZD $123` with:

```
\b(?:NZD|SGD|…)(?=\$?(?:[\W\d]|$))
```

Rust's `regex` crate has no lookaround. The assertion is split out: match the code at a word
boundary, then test what follows against an anchored pattern. Reusing a regex for that check rather
than classifying characters by hand keeps `\W` and `\d` on the engine's Unicode semantics instead of
an approximation of Python's.

Splitting it introduces a subtlety that has to be restored deliberately: **a rejected candidate must
not end the search.** The lookahead form retries at later positions implicitly, so the port iterates.
`"NZDX AUD"` yields `AUD`. And the `\b` carries more weight than it appears to — in `"USDUSD "` the
first `USD` is rejected and the second cannot match at all, so upstream returns nothing, and so does
this.

### A conditional group, which is harder

`extract_price_text` handles a euro acting as the decimal point, where `35€ 99` means `35.99`:

```python
\s*?€(\s*?)?      # euro, maybe whitespace-separated   <- group 1
\d(?(1)\d|\d*?)   # group 1 matched -> one more digit; else -> a lazy run
```

`(?(1)yes|no)` is an if-then-else with no Rust equivalent. Rather than hunt for a crate that has one,
the question was what the conditional actually decides — and group 1 comes back as exactly three
things:

| group 1 | meaning | digits after `€` |
|---|---|---|
| `''` | participated, empty | exactly 2 |
| `' '` | participated, matched | exactly 2 |
| `None` | skipped by backtracking | 1+, lazy, no whitespace consumed |

Three values, two outcomes. So the pattern splits cleanly into two ordinary regexes, tried in the
engine's own search order: leftmost start wins, and the participating arm is preferred on a tie.
That is equivalent, because a later start is only reached once every earlier one has failed for both
arms.

The distinction is load-bearing. `12€345` matches the skipped arm with three digits, while `12€ 345`
matches neither and falls through to the ordinary rule. Getting the arms the wrong way round would
swap those and still look plausible.

### Python's two-phase construction

The upstream suite subclasses `Price` and calls `super().__init__(...)`. That forces both halves of
the construction protocol to be handled:

- `#[new]` becomes `tp_new`, and `type.__call__` always routes through it — so it is what direct
  `Price(a, b, c)` construction hits.
- An `__init__` defined in `#[pymethods]` lands in the type's dict but **not** in the `tp_init` slot.
  An explicit `super().__init__(...)` finds it by name; `type.__call__` does not.

Neither alone is enough, which a spike established by experiment rather than by reading
documentation. With only `#[new]`, `super().__init__` reaches `object.__init__`, which accepts the
arguments and silently discards them — every field empty, every assertion comparing nothing. With
only `__init__`, direct construction returns empty values. Both are implemented, and `#[new]` stays
permissive because a subclass pushes its own unrelated signature through it first.

All 34 decisions — every place a literal translation would have been wrong — are written up in
[`DECISIONS.md`](DECISIONS.md).

### A bug the test suite could not find

Python's `Decimal` accepts **any** Unicode decimal digit, so `Decimal("٥")` is 5. `rust_decimal`
accepts only ASCII. Both regex engines match `\p{Nd}` for `\d`, so extraction agreed perfectly and
the divergence sat entirely in the conversion — meaning a price written in Arabic-Indic, Devanagari
or Bengali numerals **silently parsed as nothing**. No error. The amount simply vanished.

The suite never caught it, because its corpus is scraped Western storefronts and is effectively all
ASCII. The differential fuzzer caught it on its first run.

Fixed by folding Unicode digits to ASCII before parsing. Every `Nd` character sits in a contiguous
run of ten starting at its script's zero, so `tools/gen_unicode_digits.py` emits just the 68 run
starts and the value follows by subtraction. Folding happens only for the numeric conversion, so
`amount_text` keeps the original digits exactly as upstream does.

## Verified beyond the suite

Each stage was compared against upstream across a generated input matrix. Inputs *and* upstream's
answers are produced from one place, so both implementations see byte-identical cases rather than two
generators that might drift.

| Stage | Cases | Result |
|---|---:|---|
| `extract_currency_symbol` | 17,899 | identical |
| `get_decimal_separator` | 697 | identical |
| `parse_number` | 2,580 | identical |
| `extract_price_text` (main branch) | 2,744 | identical |
| `extract_price_text` (euro branch) | 1,403 | identical |
| `Price::fromstring`, real corpus | 1,184 | identical on every field |

Each is reproducible via the programs in [`examples/`](examples/), which exit non-zero on any
disagreement. `fromstring` is compared field by field, because a port can get the amount right while
quietly losing the currency and a whole-object check would hide which one drifted.

Then the cases nobody thought to write: `tools/fuzz_diff.py` builds random price-like strings, runs
them through upstream, and compares every field. Runs are reproducible — each prints its seed, and
`--seed` replays it exactly.

Last qualifying run, recorded in [`fuzz/log.txt`](fuzz/log.txt):

```
iterations : 500000
upstream   : ../price-parser @ 64e213a
elapsed    : 100.4s
result     : PASS - zero divergences
```

It never links Python into Rust — upstream runs in a separate interpreter and the two outputs are
compared field by field.

## Performance

Measured over the same 1,178 real price strings from the frozen suite.

| implementation | per price | p50 | p99 | startup | peak RSS |
|---|---:|---:|---:|---:|---:|
| upstream Python | 22.0 µs | 18.3 µs | 70.0 µs | 324 ms | n/a |
| this port, from Python | 4.6 µs | 4.5 µs | 10.6 µs | 108 ms | n/a |
| this port, native Rust | 4.7 µs | 3.5 µs | 9.5 µs | **25 ms** | n/a |

**Startup is the larger win: ~13× faster to first answer**, which matters more than throughput for
anything short-lived. Throughput is around 4×, and the tail improves by a similar margin.

Raw numbers in [`bench/results.json`](bench/results.json); the full method, including what these
figures cannot tell you, in [`bench/methodology.md`](bench/methodology.md). Reproduce with
`python tools/bench.py --upstream ../price-parser`.

### How much to trust these

Less than their precision suggests. The Python baseline is steady to about ±5%, but the Rust figures
are noisier in relative terms simply because they are smaller — across repeated samples the native
throughput swung between roughly 2.3× and 6.1× the baseline. **The two Rust paths cannot be told
apart on this hardware**: the FFI overhead is real but smaller than the machine's jitter.

Latency percentiles carry timer overhead of tens of nanoseconds against a parse of a few
microseconds — about a percent, paid by both sides. Visible in the numbers, not dominating them, and
a p99 quoted without saying so would be overclaiming.

**Peak RSS is reported as unavailable on Windows, deliberately.** Two measurement approaches both
returned a constant ~3.4 MiB regardless of workload; a child was made to allocate 200 MB and the
figure did not move. Rather than publish a number known to be wrong, the platform reports nothing.
Linux uses `getrusage`, which is reliable.

Two mistakes shaped this benchmark, both caught by numbers that made no sense. `maturin develop`
defaults to a **debug** build, ~20× slower, which made the port look five times *slower* than Python
— the module now reports its own profile and the benchmark refuses to run against debug. And timing
`cargo run` instead of the compiled binary attributed ~700 ms of cargo's freshness check to Rust
startup.

## Using it

### From Rust

```rust
use price_parser::Price;

let price = Price::fromstring(Some("$12.99"), None, None, None);
assert_eq!(price.currency.as_deref(), Some("$"));
assert_eq!(price.amount_text.as_deref(), Some("12.99"));
// amount is an exact Decimal, so money does not drift.
```

### From Python

```python
from price_parser import Price

price = Price.fromstring("$12.99")
price.amount        # Decimal('12.99')
price.currency      # '$'
price.amount_text   # '12.99'
```

Upstream is a `py.typed` package, so a type checker sees its annotations. The implementation here is
Rust and has none for a checker to read, so the wheel ships [PEP 561](https://peps.python.org/pep-0561/)
stubs to keep the port a drop-in replacement for type checking too:

```
price.amount        # Decimal | None
price.currency      # str | None
price.amount_float  # float | None
```

The stubs are hand-written, so `tests/typing/` pins each signature with `assert_type` and CI runs
`mypy --strict` over it. Without that, a method added to the Rust and forgotten in the stub would go
unnoticed.

### Building

```bash
cargo test                          # the pure-Rust core, no Python needed

python -m venv .venv
.venv/bin/pip install maturin pytest # Windows: .venv\Scripts\pip
.venv/bin/maturin develop --release
.venv/bin/python -m pytest           # both suites
```

Use `--release` when building the extension module. The debug build is roughly twenty times slower,
and the benchmark will refuse to run against it.

Comparing against upstream additionally needs `attrs`, which is upstream's only runtime dependency:
`.venv/bin/pip install attrs`.

## Repository layout

| Path | What is in it |
|---|---|
| `src/symbols.rs` | Currency matching, both tiers, and the dollar-code lookahead replacement |
| `src/text.rs` | `extract_price_text`, including the conditional-group branch |
| `src/number.rs` | Separator inference and `parse_number` |
| `src/price.rs` | The `Price` value and `fromstring` — the pure-Rust entry point |
| `src/python.rs` | The only module that touches FFI |
| `src/currencies.rs`, `src/digits.rs` | Generated tables, never hand-edited |
| `price_parser/` | The Python package: import shim, PEP 561 stubs, `py.typed` |
| `tools/` | Generators, hash verification, the fuzzer, the benchmark |
| `examples/` | Differential checkers, each exiting non-zero on disagreement |
| `tests/original/` | Upstream's suite, frozen and hashed |
| `tests/typing/` | `assert_type` checks holding the stubs to the module |

`price_parser/__init__.py` is not incidental and should not be deleted. Creating that directory
switches maturin to a mixed layout, at which point it stops generating the shim itself — so the file
has to exist for `from price_parser import Price` to work at all.

### `unsafe` count: zero

No hand-written `unsafe` exists anywhere in this crate, and that is enforced by the compiler rather
than promised:

```rust
#![cfg_attr(not(feature = "python"), forbid(unsafe_code))]
```

`forbid` cannot be overridden by an inner `allow`, so the default build fails outright if any `unsafe`
appears. It is relaxed only under the `python` feature, because PyO3's macros expand to `unsafe` at
the FFI boundary — none of it written by hand, and all of it confined to `src/python.rs`. CI reports
the count on every push.

The core also carries no PyO3 dependency at all: `cargo test` runs the full Rust suite with no Python
installed.

## Reproducing every claim

Comparisons against upstream need a checkout at the pinned revision:

```bash
git clone https://github.com/scrapinghub/price-parser
git -C price-parser checkout 64e213a46a40473ba4f8aa3b249917fdc64d8a16
```

| Claim | Command |
|---|---|
| Tests unmodified | `python tools/verify_hashes.py` |
| One commit against them | `git log --oneline -- tests/original/` |
| The suite passes | `pytest tests/original -q` |
| Generated tables are current | `python tools/gen_unicode_digits.py --check` |
| Currency tables are current | `python tools/gen_currencies.py --upstream ../price-parser/price_parser --check` |
| Stubs match the module | `mypy --strict tests/typing/` |
| 202,000 fuzz cases agree | `python tools/fuzz_diff.py --iterations 50000 --upstream ../price-parser` |
| ~4× faster | `python tools/bench.py --upstream ../price-parser` |

## Licence

- [`LICENSE`](LICENSE) — the original Scrapinghub BSD-3-Clause licence, verbatim, as clause 1
  requires.
- [`LICENSE-PORT`](LICENSE-PORT) — covers the Rust port.

The crate is deliberately **not** named to imply any association with Scrapinghub, whose BSD-3-Clause
clause 3 forbids using their name to endorse or promote a derivative work.
