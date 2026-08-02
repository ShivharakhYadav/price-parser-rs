#!/usr/bin/env python3
"""Freeze and verify the vendored original test suite.

Port Mortem requires that the source project's tests are hashed at kickoff and
never modified. Everything under ``tests/original/`` is therefore treated as
read-only evidence: this script records a SHA-256 for each file and re-checks
them on demand and in CI.

    python tools/verify_hashes.py           # verify (exit 1 on any mismatch)
    python tools/verify_hashes.py --write   # (re)generate the manifest

``--write`` is intended to be run exactly once, when the suite is first
vendored. Running it again after the tests have been touched would defeat the
entire point, so it refuses unless --force is also given.
"""

from __future__ import annotations

import argparse
import hashlib
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
ORIGINAL_DIR = REPO_ROOT / "tests" / "original"
MANIFEST = ORIGINAL_DIR / "SHA256SUMS"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


# Our own bookkeeping, not upstream material. Excluded so the manifest covers
# only genuine upstream files -- mixing our docs in would both weaken the claim
# and make routine edits to them look like tampering.
NOT_UPSTREAM = {"SHA256SUMS", "PROVENANCE.md"}

# Build artefacts. Running pytest writes __pycache__ next to the tests, and
# treating that as an untracked addition would fail the check every time the
# suite runs -- including in CI, right after the step that proves the port
# works. Only compiled caches are ignored; a stray .py file here is still
# reported, since that is exactly the kind of addition worth catching.
IGNORED_DIRS = {"__pycache__", ".pytest_cache"}
IGNORED_SUFFIXES = {".pyc", ".pyo"}


def is_artefact(path: Path) -> bool:
    return (
        any(part in IGNORED_DIRS for part in path.parts)
        or path.suffix in IGNORED_SUFFIXES
    )


def tracked_files() -> list[Path]:
    """Every upstream file under tests/original/."""
    return sorted(
        p
        for p in ORIGINAL_DIR.rglob("*")
        if p.is_file() and p.name not in NOT_UPSTREAM and not is_artefact(p)
    )


def write_manifest(force: bool) -> int:
    if MANIFEST.exists() and not force:
        print(f"refusing to overwrite {MANIFEST.name}: pass --force if you truly mean it")
        print("the manifest is the evidence that the originals are untouched")
        return 1

    files = tracked_files()
    if not files:
        print(f"no files found under {ORIGINAL_DIR}")
        return 1

    lines = [f"{sha256(p)}  {p.relative_to(ORIGINAL_DIR).as_posix()}" for p in files]
    MANIFEST.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {MANIFEST.relative_to(REPO_ROOT)} covering {len(files)} file(s):")
    for line in lines:
        print(f"  {line}")
    return 0


def verify() -> int:
    if not MANIFEST.exists():
        print(f"manifest missing: {MANIFEST}")
        return 1

    expected: dict[str, str] = {}
    for line in MANIFEST.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        digest, _, name = line.partition("  ")
        expected[name] = digest

    actual = {
        p.relative_to(ORIGINAL_DIR).as_posix(): sha256(p) for p in tracked_files()
    }

    problems: list[str] = []
    for name, digest in expected.items():
        if name not in actual:
            problems.append(f"MISSING   {name}")
        elif actual[name] != digest:
            problems.append(f"MODIFIED  {name}")
            problems.append(f"            expected {digest}")
            problems.append(f"            actual   {actual[name]}")
    for name in actual:
        if name not in expected:
            problems.append(f"UNTRACKED {name}")

    if problems:
        print("ORIGINAL TEST SUITE HAS BEEN ALTERED")
        for p in problems:
            print(f"  {p}")
        print("\nThese files must remain byte-identical to the upstream originals.")
        return 1

    print(f"OK: {len(expected)} original test file(s) verified unmodified")
    for name, digest in expected.items():
        print(f"  {digest}  {name}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="generate the manifest")
    parser.add_argument("--force", action="store_true", help="allow overwriting it")
    args = parser.parse_args()
    return write_manifest(args.force) if args.write else verify()


if __name__ == "__main__":
    sys.exit(main())
