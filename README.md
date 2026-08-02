# price-parser-rs

A Rust port of [`scrapinghub/price-parser`](https://github.com/scrapinghub/price-parser) — extract a
price amount and currency symbol from a raw text string.

> **Not affiliated with, nor endorsed by, Scrapinghub.** This is an independent port. The original
> Python implementation is copyright Scrapinghub and BSD-3-Clause licensed; see [LICENSE](LICENSE),
> retained verbatim as that licence requires.

---

## The original test suite passes, unmodified

```
1059 passed, 134 xfailed
```

That is upstream's own suite, byte-for-byte, executing against Rust.

The suite is vendored under [`tests/original/`](tests/original/) and frozen: every file is SHA-256
hashed, the manifest is committed, and `tools/verify_hashes.py` re-checks it. The tests are never
edited. They run against Rust through a [PyO3](https://pyo3.rs) extension module presenting the same
import path and API as the Python package, so `from price_parser import Price` resolves to this
crate. No adapter, no shim, no rewritten assertions.

The 134 `xfailed` are upstream's own two `XFAIL` corpora, marked `strict=True` — cases upstream does
not pass either. They are expected to fail and do.

Built for [Port Mortem — Code Resurrection 2026](https://coderesurrection.com/2026), Track D.

🚧 Benchmarks, differential fuzzing and CI still to come.

## Also verified differentially

Beyond the suite, each stage was compared against upstream across a generated input matrix — inputs
and upstream's answers produced from one place so both implementations see identical cases:

| Stage | Cases | Result |
|---|---:|---|
| `extract_currency_symbol` | 17,899 | identical |
| `get_decimal_separator` | 697 | identical |
| `parse_number` | 2,580 | identical |
| `extract_price_text` (main) | 2,744 | identical |
| `extract_price_text` (euro) | 1,403 | identical |
| `Price::fromstring`, real corpus | 1,184 | identical on every field |

Each is reproducible via the programs in [`examples/`](examples/), which exit non-zero on any
disagreement.

## Performance

Roughly **4× faster** than the Python original, over the same 1,178 real price
strings from the frozen suite.

| implementation | per price | prices/sec | speedup |
|---|---:|---:|---:|
| upstream Python | ~32 µs | ~31,000 | 1.0× |
| this port, from Python | ~8 µs | ~127,000 | ~4× |
| this port, native Rust | ~7 µs | ~146,000 | ~4× |

Reproduce with `python tools/bench.py --upstream ../price-parser`.

Read these as approximate, and the table above as a representative sample
rather than a precise result. Across repeated runs the Python-facing speedup
sat in the 3.6–4.7× band, while the native figure swung from 2.3× to 6.1× — so
the two Rust paths cannot be told apart on this hardware. The FFI overhead is
real but smaller than the machine's jitter. The Python baseline was the steady
one, at about ±5%.

The honest summary is **around 4×**, not a precise multiple. Anyone wanting a
firm number should re-run on a quiet machine.

Methodology is identical on both sides: same corpus, a warmup pass, then the
best of five rounds of sixty passes each. Best rather than mean, because the
fastest observed run is the least polluted by scheduling noise, and taking it
on both sides keeps the bias pointing the same way. Rounds are long
deliberately — a single pass takes a few milliseconds and timing that produced
a table where the FFI path appeared faster than the native call it wraps.

The extension module reports its own build profile as `price_parser.__build__`,
and the benchmark refuses to run against a debug build. `maturin develop`
defaults to debug, which is around twenty times slower and made this port look
five times *slower* than Python the first time it was measured.

## Rust API

```rust
use price_parser::Price;

let price = Price::fromstring(Some("$12.99"), None, None, None);
assert_eq!(price.currency.as_deref(), Some("$"));
assert_eq!(price.amount_text.as_deref(), Some("12.99"));
// amount is an exact Decimal, so money does not drift.
```

The core carries no PyO3 dependency and `unsafe` is forbidden outright in that build; the FFI layer
is a thin, isolated shim.

## Licence

- [`LICENSE`](LICENSE) — the original Scrapinghub BSD-3-Clause licence, verbatim.
- [`LICENSE-PORT`](LICENSE-PORT) — covers the Rust port.
