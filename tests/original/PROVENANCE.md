# Provenance of the vendored test suite

Everything in this directory is copied **byte-for-byte** from the upstream project and is
**never modified**. It is the evidence that the Rust port is validated against the real,
original tests rather than a rewrite.

| | |
|---|---|
| Upstream project | [`scrapinghub/price-parser`](https://github.com/scrapinghub/price-parser) |
| Upstream commit | `64e213a46a40473ba4f8aa3b249917fdc64d8a16` |
| Upstream commit date | 2026-03-19 12:32:46 +0100 |
| Upstream version | 0.5.1 |
| Licence | BSD-3-Clause (see `/LICENSE`, retained verbatim) |
| Vendored on | 2026-08-01 |

## Contents

| File | Lines | Test cases |
|---|---|---|
| `test_price_parsing.py` | 1,489 | 1,185 |

The suite defines eight corpora: `PRICE_PARSING_EXAMPLES`, `_2`, `_3`, `_NO_PRICE`,
`_NO_CURRENCY`, `_BUGS_CAUGHT`, `_NEW`, `_XFAIL`, `_XFAIL_CURRENCIES_TO_BE_ADDED`, and
`PRICE_PARSING_DECIMAL_SEPARATOR_EXAMPLES`.

The two `XFAIL` corpora are cases upstream itself does not pass; they remain marked as
expected failures and are not a target for this port.

## Integrity

`SHA256SUMS` records a SHA-256 for every file here. Verify at any time with:

```
python tools/verify_hashes.py
```

CI runs this **before** anything else, so a modified test file fails the build loudly rather
than quietly flattering the results.

## How these tests run against Rust

The port is compiled by [PyO3](https://pyo3.rs)/[maturin](https://www.maturin.rs) into a native
extension module that presents the same import path and public API as the Python package. The
file above therefore executes **unchanged** — its `from price_parser import Price` resolves to
Rust code. No adapter, no shim, no edited assertions.
