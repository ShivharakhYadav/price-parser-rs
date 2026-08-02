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
