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

### decoding
there is no str/string in bencode, only bytes.
so we decode bencode bytes to python bytes, since it may not be a utf8 string.

### encoding
we encode python `True` as int `1` and `False` as int 0.
