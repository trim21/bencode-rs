# A bencode serialize/deserialize library written in Rust with pyo3

## install

```shell
pip install bencode-rs
```

## basic usage

```python
import bencode_rs

assert bencode_rs.bdecode(b"d4:spaml1:a1:bee") == {b"spam": [b"a", b"b"]}

assert bencode_rs.bencode({'hello': 'world'}) == b'd5:hello5:worlde'
```

## Notice

### Decoding

| bencode type | python type |
| :----------: | :---------: |
|   integer    |    `int`    |
|    string    |   `bytes`   |
|    array     |   `list`    |
|  dictionary  |   `dict`    |

bencode has no string type, only bytes.
so we decode bencode bytes to python `bytes`, since it may not be a utf-8 string.

### Encoding

|            python type            | bencode type |
| :-------------------------------: | :----------: |
|              `bool`               | integer 0/1  |
|       `int`, `enum.IntEnum`       |   integer    |
|       `str`, `enum.StrEnum`       |    string    |
| `bytes`, `bytearray`,`memoryview` |    string    |
|   `list`, `tuple`, `NamedTuple`   |    array     |
|       `dict`, `OrderedDict`       |  dictionary  |
|            dataclasses            |  dictionary  |

Also, we encode python `True` as int `1` and `False` as int 0.

### Relaxed decoding

`bdecode` rejects dict keys that are not sorted or that are duplicated, as the spec requires.
Some trackers do not follow that rule, so `bdecode_relax` accepts such dicts:

```python
import bencode_rs

# `complete` is not sorted after `peers`
bencode_rs.bdecode_relax(b"d5:peersi1e8:completei0ee") == {
    b"peers": 1,
    b"complete": 0,
}
# the last duplicated key wins
bencode_rs.bdecode_relax(b"d1:ai1e1:ai2ee") == {b"a": 2}
```

Everything else is still validated, `bdecode_relax` only relaxes dict key ordering and uniqueness.
Don't use it to parse torrent files, duplicated keys make the info hash unstable.

### Decode depth limit

`bdecode` and `bdecode_relax` accept a keyword-only `max_depth` argument.
`0` (the default) means no limit, while a positive value caps how many nested
lists or dicts are allowed:

```python
import bencode_rs

assert bencode_rs.bdecode(b"llee", max_depth=2) == [[]]

try:
    bencode_rs.bdecode(b"llee", max_depth=1)
except bencode_rs.BencodeDecodeError:
    pass
```

Set `max_depth` when decoding untrusted input to avoid a stack overflow from
deeply nested documents. `bdecode` does not limit nesting depth by default.
