from typing import Any

import pytest

from bencode_rs import BencodeDecodeError, bdecode, bdecode_relax


def test_non_bytes_input():
    with pytest.raises(TypeError):
        bdecode("s")  # type: ignore

    with pytest.raises(TypeError):
        bdecode(1)  # type: ignore


@pytest.mark.parametrize(
    "raw",
    [
        b"",
        b"1",
        b"1:",
        b"1a:",
        b"1 :",
        b"i-0e",
        b"i01e",
        b"iae",
        b"i.e",
        b"ie",
        b"i-e",
        b"i-ae",
        b"iae",
        b"iabce",
        b"1a2:qwer",  # invalid str length
        b"i123",  # invalid int
        b"01:q",  # invalid str length
        b"10:q",  # str length too big
        b"a",
        # directory keys not sorted for {'foo': 1, 'spam': 2}
        b"di1ei2ee",
        b"d3:foo4:spam3:bari42ee",
        b"d4:spaml1:a1:be",
        b"d3:keyi1e3:keyi2ee",  # duplicated keys
        b"d-3:keyi1e3:keai2ee",  # duplicated keys
        b"di1ei1e3:keai2ee",  # duplicated keys
        b"l",
        b"lee",
        b"dee",
    ],
)
def test_bad_case(raw: bytes):
    with pytest.raises(BencodeDecodeError):
        bdecode(raw)


@pytest.mark.parametrize(
    ["raw", "expected"],
    [
        # (memoryview(b"0:"), b""),
        # (bytearray(b"0:"), b""),
        (b"0:", b""),
        (b"4:spam", b"spam"),
        (b"i-3e", -3),
        (b"i49e", 49),
        (b"i4927586304e", 4927586304),
        (b"i9223372036854775806e", 9223372036854775806),
        (b"i9223372036854775807e", 9223372036854775807),
        (b"i9223372036854775808e", 9223372036854775808),
        (b"i-9223372036854775807e", -9223372036854775807),
        (b"i-9223372036854775808e", -9223372036854775808),
        (b"i-9223372036854775809e", -9223372036854775809),
        (b"i9223372036854775808e", 9223372036854775808),  # longlong int +1
        (b"i18446744073709551616e", 18446744073709551616),  # unsigned long long +1
        (b"i-9223372036854775808e", -9223372036854775808),
        (b"i-18446744073709551616e", -18446744073709551616),
        (b"le", []),
        (b"de", {}),
        (b"l4:spam4:eggse", [b"spam", b"eggs"]),
        # (b"de", {}),
        (b"d3:cow3:moo4:spam4:eggse", {b"cow": b"moo", b"spam": b"eggs"}),
        (b"d4:spaml1:a1:bee", {b"spam": [b"a", b"b"]}),
        (b"d0:4:spam3:fooi42ee", {b"": b"spam", b"foo": 42}),
    ],
)
def test_basic(raw: bytes, expected: Any):
    assert bdecode(raw) == expected


def test_decode1():
    assert bdecode(b"d1:ad2:id20:abcdefghij0123456789e1:q4:ping1:t2:aa1:y1:qe") == {
        b"a": {b"id": b"abcdefghij0123456789"},
        b"q": b"ping",
        b"t": b"aa",
        b"y": b"q",
    }


def test_decode_bytes_length_overflow():
    """length value that would overflow usize should raise DecodeError, not panic or wrap."""
    # A very large length that exceeds any reasonable usize
    with pytest.raises(BencodeDecodeError):
        bdecode(b"99999999999999999999:abc")
    # 18446744073709551616 is usize::MAX + 1 on 64-bit
    with pytest.raises(BencodeDecodeError):
        bdecode(b"18446744073709551616:abc")


def test_decode_zero_length_bytes():
    """zero-length byte string should decode to empty bytes."""
    assert bdecode(b"0:") == b""


def test_decode_int_no_digits_after_sign():
    """'i-e' should raise DecodeError, not silently return 0."""
    with pytest.raises(BencodeDecodeError):
        bdecode(b"i-e")


# announce response from an XBT tracker, keys are not sorted
UNSORTED_ANNOUNCE = (
    b"d8:intervali1800e12:min intervali900e5:peers"
    + b"108:"
    + b"\x00" * 108
    + b"8:completei18e10:incompletei0ee"
)


def test_decode_relax_unsorted_keys():
    with pytest.raises(BencodeDecodeError):
        bdecode(UNSORTED_ANNOUNCE)

    assert bdecode_relax(UNSORTED_ANNOUNCE) == {
        b"interval": 1800,
        b"min interval": 900,
        b"peers": b"\x00" * 108,
        b"complete": 18,
        b"incomplete": 0,
    }


@pytest.mark.parametrize(
    ["raw", "expected"],
    [
        # last duplicated key wins
        (b"d3:keyi1e3:keyi2ee", {b"key": 2}),
        # duplicated key out of order
        (b"d1:ai1e1:bi2e1:ai3ee", {b"a": 3, b"b": 2}),
        # unsorted keys in nested dict
        (b"d1:ad1:ci1e1:bi2ee1:zi0ee", {b"a": {b"c": 1, b"b": 2}, b"z": 0}),
        # unsorted keys in dict inside list
        (b"ld1:bi1e1:ai2eee", [{b"b": 1, b"a": 2}]),
    ],
)
def test_decode_relax_keys(raw: bytes, expected: Any):
    with pytest.raises(BencodeDecodeError):
        bdecode(raw)

    assert bdecode_relax(raw) == expected


@pytest.mark.parametrize(
    "raw",
    [
        b"",
        b"1a:",
        b"01:q",
        b"i-0e",
        b"i-e",
        b"d3:keyi1e",  # truncated dict
        b"d-3:keyi1e3:keai2ee",  # invalid key length prefix
        b"d3:keyi1e3:keyi2e",  # truncated dict with duplicated keys
        b"d3:keyi1eeextra",  # trailing bytes
    ],
)
def test_decode_relax_still_rejects_invalid_input(raw: bytes):
    with pytest.raises(BencodeDecodeError):
        bdecode_relax(raw)
