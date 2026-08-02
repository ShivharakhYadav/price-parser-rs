"""Rust port of scrapinghub/price-parser.

Re-exports the compiled extension module. This mirrors byte-for-byte what
maturin generates for a pure-Rust project; it is written out explicitly only
because shipping PEP 561 type information alongside the module requires this
directory to exist, and maturin stops generating the shim once it does.

Keep it equivalent to maturin's version. The vendored upstream test suite
imports through here.
"""

from .price_parser import *

__doc__ = price_parser.__doc__
if hasattr(price_parser, "__all__"):
    __all__ = price_parser.__all__
