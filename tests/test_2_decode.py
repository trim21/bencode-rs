from typing import Any

import pytest

from bencode2 import bdecode, BencodeDecodeError


@pytest.mark.parametrize(
    ["raw", "expected"],
    [
        (b"i0e", 0),
        (b"i1e", 1),
        (b"i10e", 10),
        (b"i-1e", -1),
        (b"i-10e", -10),
        (b"0:", b""),
        (b"4:spam", b"spam"),
        (b"i-3e", -3),
        (b"i9223372036854775808e", 9223372036854775808),  # longlong int +1
        (b"i18446744073709551616e", 18446744073709551616),  # unsigned long long +1
        (b"i4927586304e", 4927586304),
        (b"l4:spam4:eggse", [b"spam", b"eggs"]),
        (b"d3:cow3:moo4:spam4:eggse", {b"cow": b"moo", b"spam": b"eggs"}),
        (b"d4:spaml1:a1:bee", {b"spam": [b"a", b"b"]}),
        (b"d0:4:spam3:fooi42ee", {b"": b"spam", b"foo": 42}),
        (b"d4:spam0:3:fooi42ee", {b"spam": b"", b"foo": 42}),
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


# @pytest.mark.parametrize(
#     ["raw", "expected"],
#     [
#         (b"d3:cow3:moo4:spam4:eggse", {"cow": b"moo", "spam": b"eggs"}),
#         (b"d4:spaml1:a1:bee", {"spam": [b"a", b"b"]}),
#     ],
# )
# def test_dict_str_key(raw: bytes, expected: Any):
#     assert bdecode(raw) == expected


@pytest.mark.parametrize(
    "raw",
    [
        b"i-0e",
        b"i01e",
        b"iabce",
        b"1a2:qwer",  # invalid str length
        b"01:q",  # invalid str length
        b"a",
    ],
)
def test_bad_case(raw: bytes):
    with pytest.raises(BencodeDecodeError):
        bdecode(raw)
