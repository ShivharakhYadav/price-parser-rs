# Decisions

Every non-trivial divergence between `scrapinghub/price-parser` and this port, and why it was made
that way.

The port is behaviourally complete: **every function, class and data table in upstream's package has
a counterpart here**, and upstream's own test suite passes unmodified. So there are no scope cuts to
declare. What follows is the set of places where a *literal* translation would have been wrong, and
what was done instead.

Most were found by comparing against real Python output rather than by reading code. Where that is
true, it is said.

---

## Architecture

### 1. The core carries no PyO3 dependency

`src/` is an ordinary Rust library. `cargo test` runs 70 tests with **no Python installed at all**.
PyO3 lives behind an optional `python` feature in exactly one module, `src/python.rs`.

*Why:* the deliverable is a Rust library, not a Python extension that happens to be written in Rust.
Keeping the binding optional means the port stands on its own and the FFI surface stays auditable —
one file, easy to check.

### 2. `unsafe` is forbidden by the compiler, not by promise

```rust
#![cfg_attr(not(feature = "python"), forbid(unsafe_code))]
```

`forbid` cannot be overridden by an inner `allow`, so the default build fails if any `unsafe` appears.
It is relaxed only under `python`, because PyO3's macros expand to `unsafe` at the FFI boundary.
**Hand-written `unsafe` blocks: zero.**

### 3. The extension module is named `price_parser`, matching upstream

*Why:* so upstream's `from price_parser import Price` resolves to this crate and the suite runs
**unmodified**. The cost is that upstream and this port cannot be imported into the same interpreter,
which shaped decision 29.

### 4. Data tables are generated, not transcribed

`tools/gen_currencies.py` emits `src/currencies.rs` from upstream's `_currencies.py` and `parser.py`.

*Why:* 671 entries across four tables, carrying 32 invisible `U+200F` marks between them. The
hand-ordered safe list alone is 126 entries, **89 of them non-ASCII** — 89 chances at a silent typo.
Only *literal data* is generated; the set arithmetic, ordering and regex construction in `symbols.rs`
are written by hand.

### 5. The original tests are frozen and hashed

`tests/original/` is byte-for-byte upstream, with a committed SHA-256 manifest. `.gitattributes`
marks the directory `-text` so no line-ending conversion can alter those bytes on checkout — without
that, a Windows clone would rewrite them and silently invalidate every hash.

---

## Regex constructs Rust cannot express

Rust's `regex` crate has no lookaround, backreferences or conditionals. Two upstream patterns use
them.

### 6. `_DOLLAR_REGEX`'s lookahead → match, then check the follower

```
\b(?:NZD|SGD|…)(?=\$?(?:[\W\d]|$))
```

Split into a plain match plus an anchored test of what follows. The follower check reuses a regex
(`^\$?(?:[\W\d]|$)`) rather than classifying characters by hand, so `\W` and `\d` keep the engine's
Unicode semantics instead of an approximation of Python's.

### 7. A rejected candidate must not end the search

The lookahead form retries at later positions implicitly. Split apart, that has to be deliberate —
so the port iterates. `"NZDX AUD"` yields `AUD`; a naive "find first, check, return" yields nothing.

### 8. The `\b` is load-bearing

In `"USDUSD "` the first `USD` is rejected (a letter follows) and the second cannot match at all,
because there is no word boundary mid-run. Upstream returns nothing. Dropping `\b` would wrongly
return `USD`.

### 9. The conditional group → two patterns in the engine's own search order

```python
\s*?€(\s*?)?      # euro, maybe whitespace-separated   <- group 1
\d(?(1)\d|\d*?)   # group 1 matched -> one more digit; else -> a lazy run
```

Rather than hunt for a crate supporting `(?(1)…)`, I probed what the conditional actually decides.
Group 1 returns exactly three things:

| group 1 | meaning | digits after `€` |
|---|---|---|
| `''` | participated, empty | exactly 2 |
| `' '` | participated, matched | exactly 2 |
| `None` | skipped by backtracking | 1+, lazy, no whitespace consumed |

Three values, two outcomes — so it splits into two ordinary regexes. Search order is reproduced by
running both and taking the earlier match, preferring the participating arm on a tie. That is
equivalent because a later start is only reached once every earlier one has failed for **both** arms.

Load-bearing: `12€345` matches the skipped arm with three digits, while `12€ 345` matches neither and
falls through. Swapping the arms would still look plausible and be wrong.

---

## Where Python and Rust genuinely disagree

These are the silent ones. Each would compile, pass a casual reading, and be wrong.

### 10. `len()` counts characters; `str::len()` counts bytes

Upstream sorts symbols by `len`. Using Rust's `.len()` would rank `€` (1 char, 3 bytes) alongside
`US$` (3 chars, 3 bytes) and quietly reorder the alternation. Uses `.chars().count()`.

### 11. Python's regex `\s` matches `U+001C`–`U+001F`; Rust's does not

Rust's `\s` is the Unicode `White_Space` property, which excludes the file, group, record and unit
separators. Upstream normalises `"1\x1c234"` to `"1 234"`. The class is widened to
`[\s\x{1c}-\x{1f}]`. Verified by probing CPython, not assumed.

### 12. `str.strip()` has the same gap

`parse_number` strips with a predicate matching Python's notion of whitespace rather than Rust's, so
`"\x1c1.5"` parses to `1.5` as upstream does.

### 13. Python's `$` also matches before a single trailing newline

Rust's `$` matches only at end-of-haystack. Upstream returns `'.'` for `"12.99\n"`; a direct
translation returns `None`. One trailing newline is stripped before matching — only one, and trailing
spaces are left alone, because `"12.99 "` gives `None` on both sides.

### 14. Alternation: leftmost position dominates; order decides only at ties

I had this wrong initially and a failing test corrected it. `$|US$` and `US$|$` **both** yield `"US$"`
on `"US$100"`, because at index 0 only `US$` can match. Listed order matters only between branches
matching at the *same* index — which is why upstream places `$U` at index 19 and bare `$` at 32.
Both behaviours verified against CPython and pinned by tests.

### 15. Empty and blank are not the same

Upstream guards each candidate with a bare truthiness test, so `""` is skipped but `"  "` is truthy
and still searched. Both find nothing, so collapsing them *looks* harmless — but it is a real
behavioural difference and is preserved.

### 16. Upstream's own table order is non-deterministic

Two of the three currency lists are built with `list({…})` over a set. Running it three times gave
three different orderings. Emitting that directly would produce a different generated file every run,
so both are sorted.

Safe to reorder: the lists only ever become regex alternations, and order matters solely between
candidates of *different* lengths — two distinct strings of equal length cannot both match at the
same position. Length precedence is applied separately. `CURRENCY_CODES` keeps upstream's dict
insertion order, which is already deterministic.

### 17. Thirty-two symbols carry an invisible `U+200F`

Right-to-left marks on Arabic currency symbols — 16 in the national-symbol table and 16 more in the
safe list. A generator that dropped them would look correct on inspection while failing to match real
input. Control and format characters are escaped as `\u{…}`, and a test pins the national count at 16
so a silent loss breaks the build.

### 18. `U+20BD` (₽) is absent from upstream's data entirely

RUB gives its native symbol as `"руб."`. The ruble sign reaches the matcher only via the hand-written
safe list. Pinned as a test so a future upstream change surfaces it rather than silently duplicating
an entry.

### 19. The safe-symbol list's hand-ordering is load-bearing

`US$`, `CA$`, `AU$` must precede bare `$` or the wrong symbol wins. Reproduced exactly as upstream
declares it.

---

## Numbers

### 20. Python's `Decimal` accepts any Unicode decimal digit

`Decimal("٥")` is 5. `rust_decimal` accepts only ASCII. Both regex engines match `\p{Nd}` for `\d`,
so extraction agreed and the divergence sat **entirely in the conversion** — prices in Arabic-Indic,
Devanagari or Bengali numerals silently parsed as nothing. No error; the amount simply vanished.

**Found by the differential fuzzer on its first run, not by the suite** — whose corpus is scraped
Western storefronts and effectively all ASCII.

Fixed by folding Unicode digits to ASCII before parsing. Every `Nd` character sits in a contiguous
run of ten starting at its script's zero, so `tools/gen_unicode_digits.py` emits just the 68 run
starts and the value follows by subtraction. Folding happens only for the numeric conversion, so
`amount_text` keeps the original digits exactly as upstream does.

### 21. Known, pinned divergences at `rust_decimal`'s limits

Checked against the real corpus **before** relying on the type: zero amounts exceed the 96-bit
mantissa, zero exceed scale 28, widest is `123456.789`. All comfortably inside range. Beyond it:

| Input | Upstream | Here |
|---|---|---|
| `Infinity`, `NaN` | `Decimal('Infinity')` / `Decimal('NaN')` | `None` |
| > 96-bit mantissa | exact | `None` |
| scale > 28 | `1E-29` | **rounds to zero** |

The last is the dangerous one — it fails *quietly* rather than returning `None` — so it is pinned
with an explicit test rather than left to chance. None are reachable through `fromstring`, whose
extraction yields only digits, spaces and separators.

### 22. Scale is preserved

`140.000` does not collapse to `140`. Upstream's decimal-separator tests distinguish them, and the
differential comparison is done on exact strings so scale is checked too.

### 23. Two of my own assumptions were wrong; Python settled both

`rust_decimal` **does** accept PEP 515 underscores, so `"1_000"` is not a divergence. And
`"123456.789"` parses to `123456789` unforced, because three trailing digits read as a thousands
group — upstream agrees. I had confused the corpus's widest *amount* with an *input*.

---

## The Python binding

### 24. Both halves of construction are needed

The suite subclasses `Price` and calls `super().__init__(...)`. That forces both:

- `#[new]` becomes `tp_new`; `type.__call__` always routes through it, so it is the only path direct
  `Price(a, b, c)` takes.
- An `__init__` in `#[pymethods]` lands in the type's dict but **not** the `tp_init` slot. An explicit
  `super().__init__(...)` finds it by name; `type.__call__` does not.

Established by experiment, not documentation. With only `#[new]`, `super().__init__` reaches
`object.__init__`, which accepts the arguments and **silently discards them** — every field empty,
every assertion comparing nothing. A spike confirmed exactly that split: 1 of 3 cases passed with
`__init__` alone; 3 of 3 with both.

### 25. `#[new]` is permissive by necessity

A subclass pushes its own unrelated signature through `tp_new` before `__init__` runs, so validating
arity there would break every such subclass. It takes `(*args, **kwargs)`, and `__init__` assigns all
three fields unconditionally so it overwrites whatever `tp_new` made of them.

### 26. Equality uses `__richcmp__`, not a `bool`-returning `__eq__`

So it can return `NotImplemented` for a non-exact class, as `attrs` does. Not cosmetic: it is
precisely what lets the suite's `assert parsed == example` resolve to `Example.__eq__` rather than
being answered here. A test builds a subclass with a permissive `__eq__` to prove the deferral
happens.

### 27. Amounts cross the boundary as strings

So a `decimal.Decimal` keeps its exact value **and scale**.

### 28. `__all__` is set last, and the package ships a shim

PyO3 appends each subsequently-added name to an existing `__all__`, so declaring it early leaked
`__build__` into the public exports and broke a test. It is set last.

Separately: creating `price_parser/` at the repo root — needed to ship PEP 561 stubs — makes maturin
switch to a mixed layout and **stop generating the `__init__.py` shim**. The first wheel built
without it, which would have broken importing the module entirely. The shim is now written out
explicitly. `price_parser/__init__.py` is load-bearing, not incidental.

---

## Verification

### 29. Differential testing runs upstream in a separate process

This crate's module is deliberately named `price_parser`, so upstream and the port cannot be imported
into one interpreter. Upstream therefore runs in a plain Python process and its answers are recorded;
the Rust side is checked against that table.

A side effect worth noting: **no Python is ever linked into Rust for verification** — it is process
separation, not FFI.

Recording answers rather than comparing live also means a failure can be replayed later without
re-running either side.

### 30. Inputs and expected answers are generated from one place

Both implementations see byte-identical cases. Two independent generators could drift and quietly
weaken the comparison.

### 31. `fromstring` is compared field by field

A port can get the amount right while losing the currency; a whole-object check would report one
failure without showing which field drifted.

### 32. Benchmarks report the best of several long rounds, on both sides

Best rather than mean, because the fastest observed run is least polluted by scheduling noise — and
taking the best on *both* sides keeps the bias pointing the same way. Rounds are long deliberately: a
single pass takes milliseconds, and timing that produced a table where the FFI path appeared *faster*
than the native call it wraps.

The module reports its own build profile and the benchmark **refuses to run against a debug build** —
`maturin develop` defaults to debug, which is ~20× slower and once made this port look five times
*slower* than Python.

### 33. The speedup is reported as a range, not a figure

~4×, with the honest note that the native measurement swung 2.3×–6.1× across samples on this
hardware, so the two Rust paths cannot be told apart. The Python baseline was the steady one at ±5%.

---

## Not done

### 34. No bugs claimed in the original

The differential work compares this port *against* upstream, so it finds places where **we** diverge,
not places where upstream is wrong. Twelve-odd divergences were found and fixed during development;
all were ours. No upstream bug is claimed, because none was found.
