from __future__ import annotations

import collections
from typing import Any

import pytest

from bencode_rs import BencodeEncodeError, bencode


def test_exception_when_strict():
    with pytest.raises(TypeError):
        bencode(None)


def test_encode_str():
    s = "你好"
    assert bencode(s) == "{}:".format(len(s.encode())).encode() + s.encode(), "utf8"

    s = "你好" * 4
    assert bencode(s) == "{}:".format(len(s.encode())).encode() + s.encode(), "utf8"

    coded = bencode("ThisIsAString")
    assert coded == b"13:ThisIsAString", "Failed to encode string from str."


def test_encode_int():
    coded = bencode(42)
    assert coded == b"i42e", "Failed to encode integer from int."


def test_encode_bytes():
    b = b"TheseAreSomeBytes"
    coded = bencode(b)
    s = bytes(str(len(b)), "utf-8")
    assert coded == s + b":" + b, "Failed to encode string from bytes."


def test_encode_list():
    s = ["a", "b", 3]
    coded = bencode(s)
    assert coded == b"l1:a1:bi3ee", "Failed to encode list from list."


def test_encode_tuple():
    t = ("a", "b", 3)
    coded = bencode(t)
    assert coded == b"l1:a1:bi3ee", "Failed to encode list from tuple."


def test_encode_dict():
    od = collections.OrderedDict(
        [
            ("kb", 2),
            ("ka", 1),
            ("kc", 3),
        ]
    )
    coded = bencode(od)
    assert coded == b"d2:kai1e2:kbi2e2:kci3ee", "Failed to encode dictionary from dict."


def test_encode_complex():
    od = collections.OrderedDict()
    od["KeyA"] = ["listitemA", {"k": "v"}, 3]
    od["KeyB"] = {"k": "v"}
    od["KeyC"] = 3
    od["KeyD"] = "AString"
    expected_result = (
        b"d4:KeyAl9:listitemAd1:k1:vei3ee4:KeyBd1:k1:ve4:KeyCi3e4:KeyD7:AStringe"
    )
    coded = bencode(od)
    assert coded == expected_result, "Failed to encode complex object."


def test_encode():
    assert bencode(
        {
            "_id": "5973782bdb9a930533b05cb2",
            "isActive": True,
            "balance": "$1,446.35",
            "age": 32,
            "eyeColor": "green",
            "name": "Logan Keller",
            "gender": "male",
            "company": "ARTIQ",
            "email": "logankeller@artiq.com",
            "phone": "+1 (952) 533-2258",
            "friends": [
                {"id": 0, "name": "Colon Salazar"},
                {"id": 1, "name": "French Mcneil"},
                {"id": 2, "name": "Carol Martin"},
            ],
            "favoriteFruit": "banana",
        }
    ) == (
        b"d3:_id24:5973782bdb9a930533b05cb23:agei32e7:balance9"
        b":$1,446.357:company5:ARTIQ5:email21:logankeller@artiq.c"
        b"om8:eyeColor5:green13:favoriteFruit6:banana7:friendsld2"
        b":idi0e4:name13:Colon Salazared2:idi1e4:name13:French Mc"
        b"neiled2:idi2e4:name12:Carol Martinee6:gender4:male8:isA"
        b"ctivei1e4:name12:Logan Keller5:phone17:+1 (952) 533-2258e"
    )


def test_duplicated_type_keys():
    with pytest.raises(BencodeEncodeError):
        bencode({"string_key": 1, b"string_key": 2, "1": 2})


def test_dict_int_keys():
    with pytest.raises(TypeError):
        bencode({1: 2})


@pytest.mark.parametrize(
    "case",
    [
        (-0, b"i0e"),
        (0, b"i0e"),
        (1, b"i1e"),
        (-1, b"i-1e"),
        (["", 1], b"l0:i1ee"),
        ({"": 2}, b"d0:i2ee"),
        ("", b"0:"),
        (True, b"i1e"),
        (False, b"i0e"),
        (-3, b"i-3e"),
        (4927586304, b"i4927586304e"),
        ([b"spam", b"eggs"], b"l4:spam4:eggse"),
        ({b"cow": b"moo", b"spam": b"eggs"}, b"d3:cow3:moo4:spam4:eggse"),
        ({b"spam": [b"a", b"b"]}, b"d4:spaml1:a1:bee"),
        ({}, b"de"),
        ((1, 2), b"li1ei2ee"),
        ([1, 2], b"li1ei2ee"),
        # slow path overflow c long long
        (9223372036854775808, b"i9223372036854775808e"),  # longlong int +1
        (18446744073709551616, b"i18446744073709551616e"),  # unsigned long long +1
    ],
    ids=lambda val: f"raw={val[0]!r} expected={val[1]!r}",
)
def test_basic(case: tuple[Any, bytes]):
    raw, expected = case
    assert bencode(raw) == expected


def test_recursive_object():
    a = 1
    assert bencode([a, a, a, a])
    b = "test str"
    assert bencode([b, b, b, b])
    assert bencode({b: b})

    bencode([[1, 2, 3]] * 3)

    d = {}
    d["a"] = d
    with pytest.raises(ValueError, match="circular reference found"):
        assert bencode(d)
