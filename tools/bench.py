#!/usr/bin/env python3
"""Benchmark this port against upstream over the real corpus.

    python tools/bench.py --upstream ../price-parser

Reports three figures, because only reporting the last would flatter the port:

  * upstream Python          -- the baseline
  * this port, from Python   -- what a Python user actually gets, FFI included
  * this port, native Rust   -- what a Rust caller gets

The corpus is every price string in the frozen upstream suite, so the inputs
are real scraped text rather than something chosen to look fast.

Methodology is identical on both sides: same inputs, a warmup pass, then the
**best** of several rounds. The fastest observed run is the one least polluted
by scheduling noise, and taking the best on both sides keeps the comparison
honest in the same direction.

The two Python measurements run in separate interpreters. This crate's
extension module is deliberately named `price_parser`, exactly like upstream,
so the two cannot coexist in one process.
"""

from __future__ import annotations

import argparse
import ast
import json
import subprocess
import sys
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SUITE = REPO_ROOT / "tests" / "original" / "test_price_parsing.py"
UPSTREAM_PIN = "64e213a46a40473ba4f8aa3b249917fdc64d8a16"


def load_corpus() -> list[str]:
    """Every price string in the frozen suite."""
    tree = ast.parse(SUITE.read_text(encoding="utf-8"))
    corpus: list[str] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Call) and getattr(node.func, "id", "") == "Example":
            if len(node.args) >= 2:
                try:
                    raw = ast.literal_eval(node.args[1])
                except Exception:  # noqa: BLE001
                    continue
                if isinstance(raw, str):
                    corpus.append(raw)
    return corpus


def time_parser(fromstring, corpus: list[str], rounds: int) -> float:
    """Best seconds-per-item over `rounds`, after a warmup pass."""
    for text in corpus:
        fromstring(text)
    best = float("inf")
    for _ in range(rounds):
        start = time.perf_counter()
        for text in corpus:
            fromstring(text)
        best = min(best, time.perf_counter() - start)
    return best / len(corpus)


BODY = """
import json, sys, time
sys.path.insert(0, sys.argv[1])
from price_parser import Price
corpus = json.loads(open(sys.argv[2], encoding="utf-8").read())
rounds, repeats = int(sys.argv[3]), int(sys.argv[4])
for t in corpus:
    Price.fromstring(t)
best = float("inf")
for _ in range(rounds):
    s = time.perf_counter()
    for _ in range(repeats):
        for t in corpus:
            Price.fromstring(t)
    best = min(best, time.perf_counter() - s)
print(json.dumps({"seconds_per_item": best / (len(corpus) * repeats)}))
"""


def run_python(path_entry: str, corpus_file: Path, rounds: int, repeats: int) -> float:
    """Time Price.fromstring in a fresh interpreter with `path_entry` first."""
    result = subprocess.run(
        [sys.executable, "-c", BODY, path_entry, str(corpus_file), str(rounds), str(repeats)],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        # Surface the child's own error rather than a bare CalledProcessError;
        # the usual cause is upstream's `attrs` dependency being absent, and
        # the traceback says so plainly.
        raise SystemExit(
            f"benchmark subprocess failed (path entry {path_entry!r}):\n{result.stderr.strip()}"
        )
    return json.loads(result.stdout)["seconds_per_item"]


def build_profile() -> str:
    """Whether the installed extension module is a debug or release build.

    Looks in two places because the layout differs. A wheel installs the
    compiled module directly as ``price_parser``, while ``maturin develop``
    installs an editable shim package whose ``__init__`` does
    ``from .price_parser import *`` -- and that honours ``__all__``, which
    deliberately lists only the two names upstream exports, so ``__build__``
    is reachable only on the inner module there.
    """
    import price_parser  # noqa: PLC0415

    if hasattr(price_parser, "__build__"):
        return price_parser.__build__
    inner = getattr(price_parser, "price_parser", None)
    return getattr(inner, "__build__", "unknown")


def fmt(seconds: float) -> str:
    return f"{seconds * 1e6:.2f} us"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, default=REPO_ROOT.parent / "price-parser")
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument(
        "--repeats",
        type=int,
        default=60,
        help="passes over the corpus per round; one pass is too short to time",
    )
    args = parser.parse_args()

    if not (args.upstream / "price_parser" / "parser.py").is_file():
        print(f"upstream checkout not found at {args.upstream}")
        print("  git clone https://github.com/scrapinghub/price-parser")
        print(f"  git -C price-parser checkout {UPSTREAM_PIN}")
        return 1

    # A debug extension module is roughly twenty times slower than release and
    # would make this port look five times slower than the Python it replaces.
    # That mistake produced a nonsense table once already, so refuse outright
    # rather than print numbers that need a caveat to be understood.
    build = build_profile()
    if build != "release":
        print(f"the installed extension module is a {build} build.")
        print("benchmark numbers from it are meaningless. rebuild with:")
        print("    maturin develop --release")
        return 1

    corpus = load_corpus()
    print(f"corpus : {len(corpus)} real price strings from the frozen suite")
    print(f"rounds : {args.rounds} (best taken), {args.repeats} passes each")
    print(f"parsed : {len(corpus) * args.repeats:,} prices per round")
    print(f"build  : {build}")
    print()

    corpus_json = REPO_ROOT / "bench-corpus.json"
    corpus_json.write_text(json.dumps(corpus), encoding="utf-8")

    corpus_txt = REPO_ROOT / "bench-corpus.txt"
    corpus_txt.write_text(
        "\n".join(
            t.replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n") for t in corpus
        )
        + "\n",
        encoding="utf-8",
        newline="\n",
    )

    try:
        print("timing upstream Python ...")
        upstream = run_python(str(args.upstream), corpus_json, args.rounds, args.repeats)

        print("timing this port from Python ...")
        # An empty first path entry keeps the installed extension module in
        # play rather than shadowing it with an upstream checkout.
        ours_py = run_python("", corpus_json, args.rounds, args.repeats)

        print("timing native Rust ...")
        subprocess.run(
            ["cargo", "build", "--release", "--example", "bench"],
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
        )
        out = subprocess.run(
            [
                "cargo", "run", "--release", "--quiet", "--example", "bench",
                "--", str(corpus_txt), str(args.rounds), str(args.repeats),
            ],
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
        native = json.loads(out.stdout)["seconds_per_item"]
    finally:
        corpus_json.unlink(missing_ok=True)
        corpus_txt.unlink(missing_ok=True)

    print()
    print(f"| {'implementation':<26} | {'per price':>10} | {'prices/sec':>12} | {'speedup':>8} |")
    print(f"|{'-' * 28}|{'-' * 12}|{'-' * 14}|{'-' * 10}|")
    for name, seconds in [
        ("upstream Python", upstream),
        ("this port, from Python", ours_py),
        ("this port, native Rust", native),
    ]:
        print(
            f"| {name:<26} | {fmt(seconds):>10} | {1 / seconds:>12,.0f} | "
            f"{upstream / seconds:>7.1f}x |"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
