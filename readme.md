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
|       `types.MappingProxy`        |  dictionary  |
|            dataclasses            |  dictionary  |

Also, we encode python `True` as int `1` and `False` as int 0.
