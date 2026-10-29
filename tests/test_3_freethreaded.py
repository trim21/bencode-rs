from __future__ import annotations

import subprocess
import sys
import sysconfig
import threading

import pytest

import bencode_rs


def test_import_keeps_gil_disabled():
    if not sysconfig.get_config_var("Py_GIL_DISABLED"):
        pytest.skip("not a free-threaded build")

    # a subprocess keeps the result independent of whatever the test run already imported
    code = (
        "import sys, bencode_rs;"
        "raise SystemExit(0 if sys._is_gil_enabled() is False else 1)"
    )
    result = subprocess.run(
        [sys.executable, "-W", "error::RuntimeWarning", "-c", code],
        capture_output=True,
    )

    assert result.returncode == 0, result.stderr.decode()


def test_concurrent_encode_decode():
    value = {
        b"interval": 3600,
        b"peers": b"1" * 300,
        b"num": {str(i).encode(): i for i in range(64)},
    }
    raw = bencode_rs.bencode(value)
    failures: list[str] = []
    start = threading.Barrier(8)

    def worker() -> None:
        start.wait()
        for _ in range(200):
            if bencode_rs.bencode(value) != raw:
                failures.append("encode mismatch")
            if bencode_rs.bdecode(raw) != value:
                failures.append("decode mismatch")

    threads = [threading.Thread(target=worker) for _ in range(8)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()

    assert failures == []
