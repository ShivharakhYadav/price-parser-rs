#!/usr/bin/env python3
"""Differential fuzzer: compare this port against upstream on random input.

The hand-written tests and the generated matrices both check cases someone
thought of. This checks cases nobody thought of, which is the point.

    python tools/fuzz_diff.py --iterations 50000 --upstream ../price-parser

Obtain the upstream source at the pinned revision with::

    git clone https://github.com/scrapinghub/price-parser
    git -C price-parser checkout 64e213a46a40473ba4f8aa3b249917fdc64d8a16

Runs are reproducible: every run prints its seed, and passing ``--seed`` back
replays exactly the same inputs.

Why two processes
-----------------
This crate's extension module is deliberately named ``price_parser``, the same
as upstream, so the original suite imports it unchanged. That makes importing
both into one interpreter impossible. Upstream therefore runs here, in a plain
interpreter that has never imported the Rust module, and writes its answers to
a table; the Rust side is verified separately by ``examples/check_fromstring``,
which reads that table. Comparing recorded answers rather than live calls also
means a failure can be replayed later without re-running either side.
"""

from __future__ import annotations

import argparse
import random
import string
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
UPSTREAM_PIN = "64e213a46a40473ba4f8aa3b249917fdc64d8a16"
NONE = "\\N"

# Whitespace worth exercising: ASCII, the Unicode spaces, and the C0 separators
# that Python treats as whitespace but Unicode's White_Space property does not.
WHITESPACE = [" ", "\t", "\n", "\xa0", "\u2009", "\u3000", "\x1c", "\x1f", "  "]
SEPARATORS = ["", ".", ",", "'", " ", "\xa0", "€"]
JUNK_WORDS = ["from", "only", "now", "was", "price", "free", "off", "OFF", "each", "ea"]
NOISE = list("!@#%^&*()[]{}<>/\\|;:\"?~`_-+=") + ["", "..", ",,", "%%"]


def build_amount(rng: random.Random) -> str:
    """A number, with grouping and a decimal part chosen independently."""
    digits = "".join(rng.choice(string.digits) for _ in range(rng.randint(1, 8)))
    if rng.random() < 0.5:
        group = rng.choice(SEPARATORS)
        pos = rng.randint(1, max(1, len(digits) - 1))
        digits = digits[:pos] + group + digits[pos:]
    if rng.random() < 0.6:
        decimal = rng.choice(SEPARATORS)
        # Deliberately spans the 1/2/3/4-digit boundary that decides whether a
        # separator reads as decimal or as thousands.
        frac = "".join(rng.choice(string.digits) for _ in range(rng.randint(0, 5)))
        digits = f"{digits}{decimal}{frac}"
    return digits


def build_input(rng: random.Random, symbols: list[str]) -> str:
    """Assemble something price-shaped, or occasionally something absurd."""
    roll = rng.random()

    if roll < 0.04:
        return ""
    if roll < 0.10:
        # Pure noise, no number at all.
        return "".join(rng.choice(JUNK_WORDS + NOISE + WHITESPACE) for _ in range(rng.randint(1, 5)))
    if roll < 0.14:
        # Random codepoints, to shake out anything encoding-related.
        return "".join(chr(rng.randint(32, 0x2E7F)) for _ in range(rng.randint(1, 12)))

    parts: list[str] = []
    if rng.random() < 0.7:
        parts.append(rng.choice(symbols))
        if rng.random() < 0.5:
            parts.append(rng.choice(WHITESPACE))
    parts.append(build_amount(rng))
    if rng.random() < 0.4:
        parts.append(rng.choice(WHITESPACE))
        parts.append(rng.choice(symbols))
    if rng.random() < 0.25:
        parts.append(rng.choice(NOISE))
    if rng.random() < 0.2:
        # A second number, so "the first one wins" gets exercised.
        parts.append(rng.choice(WHITESPACE) + build_amount(rng))
    if rng.random() < 0.15:
        parts.insert(0, rng.choice(JUNK_WORDS) + rng.choice(WHITESPACE))
    if rng.random() < 0.1:
        parts.append("%")
    return "".join(parts)


def encode(value: object) -> str:
    if value is None:
        return NONE
    return str(value).replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--iterations", type=int, default=20000)
    parser.add_argument("--seed", type=int, default=None)
    parser.add_argument(
        "--upstream",
        type=Path,
        default=REPO_ROOT.parent / "price-parser",
        help="path to an upstream price-parser checkout",
    )
    parser.add_argument("--keep", action="store_true", help="keep the generated table")
    args = parser.parse_args()

    if not (args.upstream / "price_parser" / "parser.py").is_file():
        print(f"upstream checkout not found at {args.upstream}")
        print("\nclone it at the pinned revision:")
        print("  git clone https://github.com/scrapinghub/price-parser")
        print(f"  git -C price-parser checkout {UPSTREAM_PIN}")
        return 1

    # Import upstream only, never this crate's module of the same name.
    sys.path.insert(0, str(args.upstream))
    from price_parser import Price as UpstreamPrice  # noqa: PLC0415

    from price_parser import parser as up  # noqa: PLC0415

    seed = args.seed if args.seed is not None else random.randrange(2**32)
    rng = random.Random(seed)
    symbols = list(up.SAFE_CURRENCY_SYMBOLS) + list(up.OTHER_CURRENCY_SYMBOLS)

    started = time.time()
    print(f"started    : {datetime.now(timezone.utc):%Y-%m-%d %H:%M:%S} UTC")
    print(f"seed       : {seed}   (pass --seed {seed} to replay)")
    print(f"iterations : {args.iterations}")
    # Relative where possible: a saved log is a shared artefact, and an absolute
    # path from whoever happened to run it is noise at best.
    try:
        shown = args.upstream.resolve().relative_to(REPO_ROOT.parent)
        shown = f"../{shown}"
    except ValueError:
        shown = args.upstream
    print(f"upstream   : {shown} @ {UPSTREAM_PIN[:7]}")

    rows = []
    for _ in range(args.iterations):
        text = build_input(rng, symbols)
        hint = build_input(rng, symbols) if rng.random() < 0.3 else None
        separator = rng.choice([None, None, None, ".", ",", "€"])

        parsed = UpstreamPrice.fromstring(text, hint, separator)
        rows.append(
            "\t".join(
                [
                    encode(text),
                    encode(hint),
                    encode(separator),
                    encode(parsed.amount),
                    encode(parsed.currency),
                    encode(parsed.amount_text),
                ]
            )
        )

    table = REPO_ROOT / f"fuzz-{seed}.tsv"
    table.write_text("\n".join(rows) + "\n", encoding="utf-8", newline="\n")

    parsed_count = sum(1 for r in rows if r.split("\t")[3] != NONE)
    print(f"with amount: {parsed_count} of {len(rows)}")
    print(f"table      : {table.name}")
    print()

    # Flush before handing the terminal to the child. Python buffers its own
    # output when piped to a file, so without this the child's result line lands
    # above this script's header in a saved log -- confusing in the one artefact
    # meant to be read as a record.
    sys.stdout.flush()
    result = subprocess.run(
        ["cargo", "run", "--quiet", "--example", "check_fromstring", "--", str(table)],
        cwd=REPO_ROOT,
        check=False,
    )
    sys.stdout.flush()

    if result.returncode == 0 and not args.keep:
        table.unlink(missing_ok=True)
    elif result.returncode != 0:
        print(f"\ntable kept for investigation: {table}")

    # Reported so a saved log can be shown to have met the 60s+ bar on its own
    # evidence, rather than the duration being asserted alongside it.
    elapsed = time.time() - started
    print()
    print(f"elapsed    : {elapsed:.1f}s")
    print(f"throughput : {args.iterations / elapsed:,.0f} cases/sec")
    print(f"finished   : {datetime.now(timezone.utc):%Y-%m-%d %H:%M:%S} UTC")
    print(f"result     : {'PASS - zero divergences' if result.returncode == 0 else 'FAIL'}")

    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
