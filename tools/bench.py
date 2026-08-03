#!/usr/bin/env python3
"""Benchmark this port against upstream over the real corpus.

    python tools/bench.py --upstream ../price-parser

Reports throughput, latency percentiles, process startup and peak memory for
three implementations, because only reporting the fastest one would flatter the
port:

  * upstream Python          -- the baseline
  * this port, from Python   -- what a Python user actually gets, FFI included
  * this port, native Rust   -- what a Rust caller gets

The corpus is every price string in the frozen upstream suite, so the inputs
are real scraped text rather than something chosen to look fast.

Writes bench/results.json. Methodology, including what these numbers cannot
tell you, is in bench/methodology.md.

The two Python measurements run in separate interpreters: this crate's
extension module is deliberately named `price_parser`, exactly like upstream,
so the two cannot coexist in one process.
"""

from __future__ import annotations

import argparse
import ast
import json
import platform
import subprocess
import sys
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SUITE = REPO_ROOT / "tests" / "original" / "test_price_parsing.py"
UPSTREAM_PIN = "64e213a46a40473ba4f8aa3b249917fdc64d8a16"
OUT_DIR = REPO_ROOT / "bench"


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


# Timed in a fresh interpreter: throughput as best-of-rounds, plus per-parse
# latencies for percentiles. Kept as source text so it can be handed to a
# subprocess with a different sys.path.
BODY = r"""
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

samples = []
for _ in range(20):
    for t in corpus:
        s = time.perf_counter()
        Price.fromstring(t)
        samples.append(time.perf_counter() - s)
samples.sort()

def pct(p):
    r = max(1, min(len(samples), int(-(-p / 100 * len(samples) // 1))))
    return samples[r - 1]

print(json.dumps({
    "seconds_per_item": best / (len(corpus) * repeats),
    "latency_samples": len(samples),
    "p50": pct(50), "p90": pct(90), "p99": pct(99), "p999": pct(99.9),
    "max": samples[-1],
}))
"""


def run_python(path_entry: str, corpus_file: Path, rounds: int, repeats: int) -> dict:
    """Time Price.fromstring in a fresh interpreter with `path_entry` first."""
    result = subprocess.run(
        [sys.executable, "-c", BODY, path_entry, str(corpus_file), str(rounds), str(repeats)],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        # Surface the child's own error rather than a bare CalledProcessError;
        # the usual cause is upstream's `attrs` dependency being absent.
        raise SystemExit(
            f"benchmark subprocess failed (path entry {path_entry!r}):\n{result.stderr.strip()}"
        )
    return json.loads(result.stdout)


def time_startup(command: list[str], rounds: int) -> float:
    """Best wall time for a process that starts, parses one price, and exits.

    Measures everything paid before the first useful answer: process launch,
    interpreter or runtime init, imports, lazy table construction, regex
    compilation. Best-of, for the same reason as throughput.
    """
    for _ in range(2):  # warm the OS file cache
        subprocess.run(command, capture_output=True, check=False)
    best = float("inf")
    for _ in range(rounds):
        start = time.perf_counter()
        subprocess.run(command, capture_output=True, check=False)
        best = min(best, time.perf_counter() - start)
    return best


def peak_rss(command: list[str]) -> int | None:
    """Peak resident memory of a child process, in bytes, or None.

    Returns None rather than guessing. A fabricated memory figure is worse than
    an absent one, and on Windows this could not be made to work.

    **Not measured on Windows.** Two approaches were tried and both returned a
    constant ~3.4 MiB regardless of the workload. That was not taken on trust:
    a child was made to allocate 50 MB and then 200 MB, and the reported figure
    did not move. Declaring the ctypes signatures properly -- `OpenProcess`
    returns a 64-bit `HANDLE` that ctypes otherwise truncates to `c_int` -- was
    necessary but not sufficient. Rather than ship a number known to be wrong,
    the platform reports nothing and `bench/methodology.md` says why.

    Neither implementation reads its own RSS: doing so from Rust would mean an
    unsafe FFI call, and the zero-unsafe guarantee is worth more than a memory
    figure.

    On Linux and macOS `getrusage(RUSAGE_CHILDREN)` is reliable, so CI reports
    a real number there.
    """
    if sys.platform == "win32":
        return None

    try:
        import resource  # noqa: PLC0415

        before = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
        subprocess.run(command, capture_output=True, check=False)
        after = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
        peak = max(after, before)
        # Linux reports kilobytes; macOS reports bytes.
        return peak * 1024 if sys.platform.startswith("linux") else peak
    except Exception:  # noqa: BLE001
        return None


def us(seconds: float | None) -> str:
    return "n/a" if seconds is None else f"{seconds * 1e6:.2f}"


def mib(size: int | None) -> str:
    return "n/a" if size is None else f"{size / (1024 * 1024):.1f} MiB"


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
            ["cargo", "build", "--release", "--example", "bench", "--example", "startup"],
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
        native = json.loads(out.stdout)

        print("measuring startup and peak memory ...")
        # The compiled binary directly, never `cargo run` -- cargo's freshness
        # check added ~700ms and would have been reported as Rust startup cost,
        # making the port look five times slower to start than CPython.
        suffix = ".exe" if sys.platform == "win32" else ""
        native_bin = REPO_ROOT / "target" / "release" / "examples" / f"startup{suffix}"
        if not native_bin.is_file():
            raise SystemExit(f"built binary not found: {native_bin}")

        one_liner = "from price_parser import Price; Price.fromstring('$12.99')"
        hold = "import time; time.sleep(0.4)"
        commands = {
            "startup": {
                "upstream": [
                    sys.executable, "-c",
                    f"import sys; sys.path.insert(0, {str(args.upstream)!r}); {one_liner}",
                ],
                "ours_py": [sys.executable, "-c", one_liner],
                "native": [str(native_bin)],
            },
            "rss": {
                "upstream": [
                    sys.executable, "-c",
                    f"import sys; sys.path.insert(0, {str(args.upstream)!r}); {one_liner}; {hold}",
                ],
                "ours_py": [sys.executable, "-c", f"{one_liner}; {hold}"],
                "native": [str(native_bin), "--hold", "400"],
            },
        }

        for key, entry in (("upstream", upstream), ("ours_py", ours_py), ("native", native)):
            entry["startup_seconds"] = time_startup(commands["startup"][key], args.rounds)
            entry["peak_rss_bytes"] = peak_rss(commands["rss"][key])
    finally:
        corpus_json.unlink(missing_ok=True)
        corpus_txt.unlink(missing_ok=True)

    rows = [
        ("upstream Python", upstream),
        ("this port, from Python", ours_py),
        ("this port, native Rust", native),
    ]

    print()
    header = f"| {'implementation':<24} | {'per price':>9} | {'p50':>7} | {'p99':>8} | {'startup':>8} | {'peak RSS':>9} |"
    print(header)
    print("|" + "-" * 26 + "|" + "-" * 11 + "|" + "-" * 9 + "|" + "-" * 10 + "|" + "-" * 10 + "|" + "-" * 11 + "|")
    for name, d in rows:
        print(
            f"| {name:<24} | {us(d['seconds_per_item']):>7}us | {us(d['p50']):>5}us | "
            f"{us(d['p99']):>6}us | {d['startup_seconds'] * 1000:>6.1f}ms | {mib(d['peak_rss_bytes']):>9} |"
        )

    OUT_DIR.mkdir(exist_ok=True)
    results = {
        "corpus": {
            "source": "tests/original/test_price_parsing.py",
            "price_strings": len(corpus),
        },
        "method": {
            "rounds": args.rounds,
            "passes_per_round": args.repeats,
            "aggregate": "best of rounds",
            "latency_samples_per_impl": upstream["latency_samples"],
            "build": build,
        },
        "platform": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
            "python": platform.python_version(),
        },
        "results": {name: d for name, d in rows},
    }
    (OUT_DIR / "results.json").write_text(
        json.dumps(results, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n"
    )
    print()
    print(f"wrote {(OUT_DIR / 'results.json').relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
