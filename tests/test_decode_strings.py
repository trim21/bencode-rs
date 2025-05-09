from bencode_rs import bdecode, bencode

def test_decode_keys():
    value = {"key": "value", "other": {"key": [1,2,3]}}
    encoded = bencode(value)
    decoded = bdecode(encoded, decode_keys = [b'key'])

    assert "key" in decoded
    assert b"other" in decoded
    assert "key" in decoded[b"other"]

