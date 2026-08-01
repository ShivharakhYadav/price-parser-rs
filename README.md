# price-parser-rs

A Rust port of [`scrapinghub/price-parser`](https://github.com/scrapinghub/price-parser) — extract a
price amount and currency symbol from a raw text string.

> **Not affiliated with, nor endorsed by, Scrapinghub.** This is an independent port. The original
> Python implementation is copyright Scrapinghub and BSD-3-Clause licensed; see [LICENSE](LICENSE),
> retained verbatim as that licence requires.

---

## Status

🚧 **Work in progress.** Built for [Port Mortem — Code Resurrection 2026](https://coderesurrection.com/2026),
Track D (Python → Rust).

## The goal

The port is validated by running the **original Python test suite, completely unmodified**, against
the Rust implementation via [PyO3](https://pyo3.rs).

The upstream suite — **1,185 test cases** — is vendored under [`tests/original/`](tests/original/)
and frozen: every file is SHA-256 hashed, the manifest is committed, and
`tools/verify_hashes.py` re-checks it. The tests are never edited. They are executed against Rust
code through a native extension module that presents the same import path and API as the Python
package.

Progress toward that goal is tracked in the commit history.

## Licence

- [`LICENSE`](LICENSE) — the original Scrapinghub BSD-3-Clause licence, verbatim.
- [`LICENSE-PORT`](LICENSE-PORT) — covers the Rust port.
