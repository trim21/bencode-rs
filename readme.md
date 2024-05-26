# A bencode serialize/deserialize library writte in Rust

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
