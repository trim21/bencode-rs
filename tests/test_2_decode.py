from typing import Any

import pytest

from bencode_rs import BencodeDecodeError, bdecode


def test_non_bytes_input():
    with pytest.raises(TypeError):
        bdecode("s")  # type: ignore

    with pytest.raises(TypeError):
        bdecode(1)  # type: ignore


@pytest.mark.parametrize(
    ["raw", "expected"],
    [
        (b"i0e", 0),
        (b"i1e", 1),
        (b"i-123e", -123),
        (b"i20022e", 20022),
        (b"i-20022e", -20022),
    ],
)
def test_decode_int(raw, expected):
    assert bdecode(raw) == expected


@pytest.mark.parametrize(
    "raw",
    [
        b"i-0e",
        b"i-01e",
        b"i01e",
        b"i01",
        b"ie",
        b"i",
    ],
)
def test_decode_int_throw(raw):
    with pytest.raises(BencodeDecodeError):
        bdecode(raw)


def test_decode_str():
    r = bytes.fromhex("0868de397935c05398eee852b7cf76a5b9801f90")
    assert len(r) == 20
    assert bdecode(b"20:" + r) == r

    with pytest.raises(BencodeDecodeError):
        bdecode(b"1:")
    with pytest.raises(BencodeDecodeError):
        bdecode(b"2:")

    assert bdecode(b"0:") == b""


@pytest.mark.parametrize(
    "raw",
    [
        b"i123",  # invalid int
        b"i-0e",
        b"i01e",
        b"iabce",
        b"1a2:qwer",  # invalid str length
        b"01:q",  # invalid str length
        b"10:q",  # str length too big
        b"a",
        b"l",
        b"lll",
        # directory keys not sorted for {'foo': 1, 'spam': 2}
        b"d3:foo4:spam3:bari42ee",
    ],
)
def test_bad_case(raw: bytes):
    with pytest.raises(BencodeDecodeError):
        bdecode(raw)


@pytest.mark.parametrize(
    ["raw", "expected"],
    [
        (b"le", []),
        (b"llee", [[]]),
        (b"lli1eelee", [[1], []]),
    ],
)
def test_list(raw: bytes, expected: Any):
    assert bdecode(raw) == expected


@pytest.mark.parametrize(
    ["raw", "expected"],
    [
        (b"i1e", 1),
        (b"i4927586304e", 4927586304),
        (b"0:", b""),
        (b"4:spam", b"spam"),
        (b"i-3e", -3),
        (b"i-9223372036854775808e", -9223372036854775808),  # longlong int +1
        (b"i9223372036854775808e", 9223372036854775808),  # longlong int +1
        (b"i18446744073709551616e", 18446744073709551616),  # unsigned long long +1
        # long long int range -9223372036854775808, 9223372036854775807
        (b"i-9223372036854775808e", -9223372036854775808),
        (b"i9223372036854775808e", 9223372036854775808),
        (b"le", []),
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


#
#
# @pytest.mark.parametrize(
#     ["raw", "expected"],
#     [
#         (b"d3:cow3:moo4:spam4:eggse", {"cow": b"moo", "spam": b"eggs"}),
#         (b"d4:spaml1:a1:bee", {"spam": [b"a", b"b"]}),
#     ],
# )
# def test_dict_str_key(raw: bytes, expected: Any):
#     assert bdecode(raw, str_key=True) == expected
