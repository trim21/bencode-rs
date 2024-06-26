import random
import sys
import time
import tracemalloc
import gc
import secrets

from bencode_rs import BencodeEncodeError, bencode, bdecode, BencodeDecodeError

help(bencode)
help(bdecode)

sys.set_int_max_str_digits(20000)

tracemalloc.start()

while True:
    s = b"100:" + secrets.token_bytes(10)

    for c in [i for i in range(5000)]:

        class C:
            pass

        try:
            bdecode(s)
        except BencodeDecodeError:
            pass

        try:
            bencode([1, 2, "a", b"b", None])
        except TypeError:
            pass

        try:
            bencode([1, 2, "a", b"b", C()])
        except TypeError:
            pass

        try:
            bencode({"0": s, "2": [True, C()], "3": None})
        except TypeError:
            pass

        try:
            bencode({"1": C()})
        except TypeError:
            pass

    gc.collect()
    v = tracemalloc.get_tracemalloc_memory()
    print(v)
    if v > 10610992:
        time.sleep(1000)
        sys.exit(1)
