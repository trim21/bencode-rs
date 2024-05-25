from typing import Any

from ._bencode import encode, BencodeEncodeError, decode, BencodeDecodeError


def bencode(obj: Any, /) -> bytes:
    return encode(obj)


def bdecode(obj: bytes, /) -> Any:
    return decode(obj)


__all__ = [
    "bencode",
    "BencodeEncodeError",
    "bdecode",
    "BencodeDecodeError",
]
