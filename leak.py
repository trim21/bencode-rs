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

    class C:
        pass

    s = b"100:" + secrets.token_bytes(10)
    # t = bencode([s, s, s])
    # i = 0
    for c in [i for i in range(5000)]:
        # i += 1
        try:
            bdecode(s)
        except BencodeDecodeError:
            pass
        try:
            bencode([1, 2, "a", b"b", C(), None])
        except TypeError:
            pass

        # bencode(True)

    gc.collect()
    v = tracemalloc.get_tracemalloc_memory()
    print(v)
    if v > 10610992:
        time.sleep(1000)
        sys.exit(1)
