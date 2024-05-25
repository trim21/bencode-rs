import random
import sys
import time
import tracemalloc
import gc
import secrets
from bencode2 import BencodeEncodeError, bencode, bdecode

sys.set_int_max_str_digits(20000)

s = tracemalloc.start()

while True:
    s = secrets.token_bytes(100)
    t = bencode([s, s, s])
    for c in [i for i in range(5000)]:
        bdecode(t)

    gc.collect()
    v = tracemalloc.get_tracemalloc_memory()
    print(v)
    if v > 10610992 * 2:
        time.sleep(1000)
        sys.exit(1)
