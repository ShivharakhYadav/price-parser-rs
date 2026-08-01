#!/usr/bin/env python3
"""Generate ``src/currencies.rs`` from the upstream package.

The upstream project carries ~1,700 lines of currency data in
``_currencies.py``, plus a hand-ordered 126-entry ``SAFE_CURRENCY_SYMBOLS``
list in ``parser.py``. Transcribing either by hand would be slow and quietly
error-prone -- 89 of those 126 entries are non-ASCII and 16 carry an invisible
U+200F RIGHT-TO-LEFT MARK -- so both are generated instead, and this script is
committed alongside its output as the record of how.

Only literal data is generated. The logic that consumes it (set arithmetic,
length ordering, regex construction) is written by hand in ``src/symbols.rs``.

Usage::

    python tools/gen_currencies.py --upstream path/to/price_parser

Obtain the upstream source at the pinned revision with::

    git clone https://github.com/scrapinghub/price-parser
    git -C price-parser checkout 64e213a46a40473ba4f8aa3b249917fdc64d8a16

Determinism
-----------
Upstream builds two of the three lists via ``list({...})`` over a set, so their
order changes between interpreter runs (string hashing is randomised). Emitting
that directly would produce a different file every time. Both are therefore
sorted here.

Ordering is safe to change. The lists are only ever compiled into regex
alternations, and alternation order matters solely between candidates of
*different* lengths -- two distinct strings of equal length cannot both match at
the same position. Length precedence is applied separately in ``symbols.rs``.
"""

from __future__ import annotations

import argparse
import ast
import importlib.util
import shutil
import subprocess
import sys
import unicodedata
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT = REPO_ROOT / "src" / "currencies.rs"
UPSTREAM_PIN = "64e213a46a40473ba4f8aa3b249917fdc64d8a16"


def load_upstream(source: Path):
    """Import the upstream module straight from its file path."""
    spec = importlib.util.spec_from_file_location("_upstream_currencies", source)
    if spec is None or spec.loader is None:
        raise SystemExit(f"cannot load a Python module from {source}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def extract_list_literal(source: Path, name: str) -> list[str]:
    """Pull a module-level list-of-str literal out of a Python file.

    Read with ``ast`` rather than by importing, so no upstream dependency
    (``attrs``) has to be installed and no module-level code runs.
    """
    tree = ast.parse(source.read_text(encoding="utf-8"))
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(
            getattr(t, "id", None) == name for t in node.targets
        ):
            return ast.literal_eval(node.value)
        if isinstance(node, ast.AnnAssign) and getattr(node.target, "id", None) == name:
            if node.value is not None:
                return ast.literal_eval(node.value)
    raise SystemExit(f"could not find {name} in {source}")


def rust_str(value: str) -> str:
    r"""Render a Python str as a Rust string literal.

    Printable non-ASCII is emitted as-is (Rust source is UTF-8), which keeps the
    table readable. Control and format characters are escaped: several national
    symbols end in U+200F RIGHT-TO-LEFT MARK, and leaving those invisible in the
    source invites an editor or a careless edit to silently drop them.
    """
    out = ['"']
    for ch in value:
        if ch == "\\":
            out.append("\\\\")
        elif ch == '"':
            out.append('\\"')
        elif unicodedata.category(ch).startswith("C"):
            out.append(f"\\u{{{ord(ch):04x}}}")
        else:
            out.append(ch)
    out.append('"')
    return "".join(out)


def render_array(name: str, doc: str, values: list[str]) -> str:
    body = "".join(f"    {rust_str(v)},\n" for v in values)
    return f"/// {doc}\n///\n/// {len(values)} entries.\npub const {name}: &[&str] = &[\n{body}];\n"


def rustfmt(path: Path) -> bool:
    """Format ``path`` in place. Returns False if rustfmt is unavailable.

    The generator formats its own output so that regeneration is idempotent:
    without this, ``cargo fmt --check`` and ``--check`` here would disagree
    forever, since rustfmt repacks the arrays that this script emits one entry
    per line.
    """
    if shutil.which("rustfmt") is None:
        return False
    subprocess.run(
        ["rustfmt", "--edition", "2021", str(path)],
        check=True,
        capture_output=True,
    )
    return True


def upstream_revision(source: Path) -> str:
    """Best-effort git revision of the upstream checkout, for provenance."""
    try:
        rev = subprocess.run(
            ["git", "-C", str(source.parent), "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
        return rev or UPSTREAM_PIN
    except (subprocess.CalledProcessError, OSError):
        return UPSTREAM_PIN


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--upstream",
        type=Path,
        required=True,
        help="path to the upstream price_parser package directory",
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--check",
        action="store_true",
        help="regenerate and fail if the output differs (for CI)",
    )
    args = parser.parse_args()

    currencies_py = args.upstream / "_currencies.py"
    parser_py = args.upstream / "parser.py"
    missing = [p for p in (currencies_py, parser_py) if not p.is_file()]
    if missing:
        for p in missing:
            print(f"upstream source not found: {p}")
        print("\nclone it at the pinned revision:")
        print("  git clone https://github.com/scrapinghub/price-parser")
        print(f"  git -C price-parser checkout {UPSTREAM_PIN}")
        print("  python tools/gen_currencies.py --upstream price-parser/price_parser")
        return 1

    upstream = load_upstream(currencies_py)
    rev = upstream_revision(currencies_py)
    safe = extract_list_literal(parser_py, "SAFE_CURRENCY_SYMBOLS")

    # Dict key order, which is insertion order and already deterministic.
    codes = list(upstream.CURRENCY_CODES)
    # Set-derived upstream, so unordered. Sort for a reproducible build.
    symbols = sorted(upstream.CURRENCY_SYMBOLS)
    national = sorted(upstream.CURRENCY_NATIONAL_SYMBOLS)

    header = f"""//! Currency data tables.
//!
//! @generated by `tools/gen_currencies.py` -- DO NOT EDIT BY HAND.
//!
//! Derived from [`scrapinghub/price-parser`][upstream] at revision
//! `{rev}`, BSD-3-Clause. Regenerate with:
//!
//! ```text
//! python tools/gen_currencies.py --source path/to/price_parser/_currencies.py
//! ```
//!
//! `CURRENCY_CODES` preserves upstream's dict insertion order. The other two
//! are built from Python sets upstream and so have no stable order of their
//! own; they are sorted here so this file is reproducible. See the generator
//! for why reordering cannot affect matching.
//!
//! [upstream]: https://github.com/scrapinghub/price-parser

"""

    safe_doc = (
        "Symbols treated as unambiguous currency indicators wherever they "
        "appear.\n///\n/// Upstream hand-orders this list and the order is "
        "load-bearing: multi-character\n/// variants such as `US$` and `CA$` "
        "must be tried before bare `$`, or the\n/// wrong symbol wins. "
        "Reproduced exactly as upstream declares it."
    )

    parts = [
        header,
        render_array(
            "CURRENCY_CODES",
            "ISO 4217 currency codes, in upstream declaration order.",
            codes,
        ),
        "\n",
        render_array(
            "CURRENCY_SYMBOLS", "Main currency symbols, sorted for determinism.", symbols
        ),
        "\n",
        render_array(
            "CURRENCY_NATIONAL_SYMBOLS",
            "National currency symbols and alternates, sorted for determinism.",
            national,
        ),
        "\n",
        render_array("SAFE_CURRENCY_SYMBOLS", safe_doc, safe),
    ]
    generated = "".join(parts)

    if args.check:
        if not args.output.exists():
            print(f"{args.output} does not exist")
            return 1
        # Compare against a formatted rendering, since that is what gets
        # committed.
        tmp = args.output.with_suffix(".rs.check")
        tmp.write_text(generated, encoding="utf-8")
        try:
            rustfmt(tmp)
            stale = tmp.read_text(encoding="utf-8") != args.output.read_text(
                encoding="utf-8"
            )
        finally:
            tmp.unlink(missing_ok=True)
        if stale:
            print(f"{args.output} is stale -- re-run tools/gen_currencies.py")
            return 1
        print(f"{args.output.relative_to(REPO_ROOT)} is up to date")
        return 0

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(generated, encoding="utf-8")
    formatted = rustfmt(args.output)

    print(f"wrote {args.output.relative_to(REPO_ROOT)}")
    print(f"  upstream revision        : {rev}")
    print(f"  CURRENCY_CODES           : {len(codes)}")
    print(f"  CURRENCY_SYMBOLS         : {len(symbols)}")
    print(f"  CURRENCY_NATIONAL_SYMBOLS: {len(national)}")
    print(f"  SAFE_CURRENCY_SYMBOLS    : {len(safe)}")
    if not formatted:
        print("  (rustfmt not found; output left unformatted)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
