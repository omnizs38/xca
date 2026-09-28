"""Dependency-free ctypes bindings for the XCA shared library."""

from __future__ import annotations
import ctypes
import os
from pathlib import Path
import sys


class _Buffer(ctypes.Structure):
    _fields_ = [("data", ctypes.POINTER(ctypes.c_uint8)), ("len", ctypes.c_size_t)]


def _library_names() -> list[str]:
    if sys.platform == "win32":
        return ["xca.dll"]
    if sys.platform == "darwin":
        return ["libxca.dylib"]
    return ["libxca.so"]


def _load_library() -> ctypes.CDLL:
    candidates: list[Path] = []
    configured = os.environ.get("XCA_LIBRARY")
    if configured:
        candidates.append(Path(configured))
    root = Path(__file__).resolve().parent.parent
    for name in _library_names():
        candidates.extend((root / "target" / "release" / name, Path(name)))
    for candidate in candidates:
        try:
            return ctypes.CDLL(str(candidate))
        except OSError:
            pass
    raise RuntimeError("XCA shared library not found; set XCA_LIBRARY or run cargo build --release")


_lib = _load_library()
_lib.xca_version.restype = ctypes.c_char_p
_lib.xca_error_string.argtypes = [ctypes.c_int32]
_lib.xca_error_string.restype = ctypes.c_char_p
_lib.xca_compress.argtypes = [ctypes.POINTER(ctypes.c_uint8), ctypes.c_size_t, ctypes.POINTER(_Buffer)]
_lib.xca_compress.restype = ctypes.c_int32
_lib.xca_decompress.argtypes = [ctypes.POINTER(ctypes.c_uint8), ctypes.c_size_t, ctypes.POINTER(_Buffer)]
_lib.xca_decompress.restype = ctypes.c_int32
_lib.xca_buffer_free.argtypes = [ctypes.POINTER(_Buffer)]


def version() -> str:
    return _lib.xca_version().decode("ascii")


def _call(function: object, data: bytes) -> bytes:
    source = (ctypes.c_uint8 * len(data)).from_buffer_copy(data)
    output = _Buffer()
    code = function(source, len(data), ctypes.byref(output))
    if code != 0:
        raise RuntimeError(_lib.xca_error_string(code).decode("utf-8"))
    try:
        return ctypes.string_at(output.data, output.len)
    finally:
        _lib.xca_buffer_free(ctypes.byref(output))


def compress(data: bytes) -> bytes:
    return _call(_lib.xca_compress, data)


def decompress(data: bytes) -> bytes:
    return _call(_lib.xca_decompress, data)
