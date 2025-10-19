use std::borrow::Cow;

use pyo3::exceptions::PyTypeError;
use pyo3::ffi::PyLong_FromString;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};
use pyo3::{create_exception, PyResult, Python};

create_exception!(
    bencode_rs,
    BencodeDecodeError,
    pyo3::exceptions::PyException
);

type DecodeError = BencodeDecodeError;

#[pyfunction]
#[pyo3(text_signature = "(b: Bytes, /)")]
pub fn bdecode(b: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let Ok(buf) = b.cast::<PyBytes>() else {
        return Err(PyTypeError::new_err("can only decode bytes"));
    };

    let size = buf.len()?;

    if size == 0 {
        return Err(DecodeError::new_err("empty bytes"));
    }

    let mut ctx = Decoder {
        bytes: buf.as_bytes(),
        index: 0,
        py: b.py(),
    };

    match ctx.decode_any() {
        Ok(object) => {
            if ctx.index != size {
                return Err(DecodeError::new_err(format!(
                    "invalid bencode data, top level value end at index {} but total bytes length {}",
                    ctx.index+1, size
                )));
            }
            Ok(object)
        }
        Err(err) => Err(err),
    }
}

struct Decoder<'a> {
    // str_key: bool,
    bytes: &'a [u8],
    index: usize, // any torrent file larger than 4GiB?
    py: Python<'a>,
}

impl<'a> Decoder<'a> {
    fn decode_any(&mut self) -> Result<Py<PyAny>, PyErr> {
        match self.current_byte()? {
            b'i' => self.decode_int(),
            b'0'..=b'9' => {
                let bytes = self.decode_bytes()?;

                Ok(PyBytes::new(self.py, bytes).unbind().into_any())
            }
            b'l' => {
                let list = self.decode_list()?;

                Ok(list.into_any())
            }
            b'd' => self.decode_dict(),
            _ => Err(DecodeError::new_err("invalid leading byte")),
        }
    }

    fn decode_bytes(&mut self) -> Result<&'a [u8], PyErr> {
        let index_sep = match self.bytes[self.index..].iter().position(|&b| b == b':') {
            Some(i) => i,
            None => {
                return Err(DecodeError::new_err(format!(
                    "invalid bytes, missing length separator: index {}",
                    self.index
                )));
            }
        } + self.index;

        if self.bytes[self.index] == b'0' && self.index + 1 != index_sep {
            return Err(DecodeError::new_err(format!(
                "invalid bytes length, leading '0' found at index {}",
                self.index
            )));
        }

        let mut len: usize = 0;
        for c in &self.bytes[self.index..index_sep] {
            if *c < b'0' || *c > b'9' {
                return Err(DecodeError::new_err(format!(
                    "invalid bytes length, found {} at index {}",
                    c, self.index
                )));
            }
            len = len * 10 + (c - b'0') as usize;
        }

        let bytes_start: usize = index_sep + 1;
        let bytes_end: usize = bytes_start + len - 1;

        if bytes_end >= self.bytes.len() {
            return Err(DecodeError::new_err(format!(
                "invalid bytes length, buffer overflow to {}: index {}, len {}",
                bytes_end, self.index, len
            )));
        }

        self.index = bytes_end + 1;

        let str_buff: &[u8] = self.bytes[bytes_start..=bytes_end].as_ref();

        Ok(str_buff)
    }

    fn decode_int(&mut self) -> Result<Py<PyAny>, PyErr> {
        let index_e = match self.bytes[self.index..].iter().position(|&b| b == b'e') {
            Some(i) => i,
            None => return Err(DecodeError::new_err("invalid int")),
        } + self.index;

        if index_e == self.index + 1 {
            return Err(DecodeError::new_err(format!(
                "invalid int, found 'ie' at index: {}",
                self.index
            )));
        }

        let mut sign = 1;

        // i1234e
        // i-1234e
        //  ^ index
        self.index += 1;

        let mut num_start = self.index;

        match self.bytes[self.index] {
            b'-' => {
                if self.bytes[self.index + 1] == b'0' {
                    return Err(DecodeError::new_err(format!(
                        "invalid int, '-0' found at {}",
                        self.index
                    )));
                }
                num_start += 1;
                sign = -1;
            }
            b'0' => {
                if self.index + 1 != index_e {
                    return Err(DecodeError::new_err(format!(
                        "invalid int, non-zero int should not start with '0'. found at {}",
                        self.index
                    )));
                }
            }
            _ => {}
        }

        for c in &self.bytes[num_start..index_e] {
            if !(b'0' <= *c && *c <= b'9') {
                return Err(DecodeError::new_err(format!(
                    "invalid int, '{}' found at {}",
                    *c as char, self.index
                )));
            }
        }

        if sign < 0 {
            let mut val: i64 = 0;

            for c_char in &self.bytes[num_start..index_e] {
                let c = *c_char - b'0';
                val = match val
                    .checked_mul(10)
                    .and_then(|v| v.checked_add(i64::from(c)))
                {
                    Some(v) => v,
                    None => {
                        return self.decode_int_slow(index_e);
                    }
                }
            }

            val = match val.checked_mul(-1) {
                Some(v) => v,
                None => {
                    // slow path to build PyLong with python
                    return self.decode_int_slow(index_e);
                }
            };

            self.index = index_e + 1;
            return Ok(val.into_pyobject(self.py)?.unbind().into_any());
        }

        let mut val: u64 = 0;

        for c_char in &self.bytes[num_start..index_e] {
            let c = *c_char - b'0';
            val = match val
                .checked_mul(10)
                .and_then(|v| v.checked_add(u64::from(c)))
            {
                Some(v) => v,
                None => {
                    return self.decode_int_slow(index_e);
                }
            }
        }

        self.index = index_e + 1;
        Ok(val.into_pyobject(self.py)?.unbind().into_any())
    }

    // support int may overflow i128/u128
    fn decode_int_slow(&mut self, index_e: usize) -> Result<Py<PyAny>, PyErr> {
        let s = &self.bytes[self.index..index_e];

        self.index = index_e + 1;

        let c_str = std::ffi::CString::new(s)?;

        unsafe {
            let ptr = PyLong_FromString(c_str.as_ptr(), std::ptr::null_mut(), 10);
            Py::from_owned_ptr_or_err(self.py, ptr)
        }
    }

    fn decode_list(&mut self) -> PyResult<Py<PyAny>> {
        self.index += 1;
        let mut l = smallvec::SmallVec::<[Py<PyAny>; 8]>::new();

        loop {
            match self.bytes.get(self.index) {
                None => {
                    return Err(DecodeError::new_err(
                        "unexpected end when parsing list".to_string(),
                    ));
                }
                Some(b'e') => break,
                Some(_) => {
                    l.push(self.decode_any()?);
                }
            }
        }

        self.index += 1;

        Ok(PyList::new(self.py, l)?.unbind().into_any())
    }

    fn decode_dict(&mut self) -> Result<Py<PyAny>, PyErr> {
        self.index += 1;

        let d = PyDict::new(self.py);
        let mut last_key: Option<Cow<[u8]>> = None;
        loop {
            match self.bytes.get(self.index) {
                // unexpected data end
                None => return Err(DecodeError::new_err("bytes end when decoding dict")),
                // loop end
                Some(b'e') => break,
                Some(_) => {
                    let key = self.decode_bytes()?;
                    let value = self.decode_any()?;

                    let ck = Cow::from(key);
                    if let Some(lk) = last_key {
                        if lk > ck {
                            return Err(DecodeError::new_err(format!(
                                "dict key not sorted. index {}",
                                self.index
                            )));
                        }

                        if lk == ck {
                            return Err(DecodeError::new_err(format!(
                                "duplicated dict key found: index {}",
                                self.index
                            )));
                        }
                    }
                    d.set_item(ck.clone(), value)?;
                    // map.insert(ck.clone(), value);
                    last_key = Some(ck);
                }
            }
        }

        self.index += 1;
        Ok(d.into())
    }

    fn current_byte(&self) -> Result<u8, PyErr> {
        match self.bytes.get(self.index) {
            None => Err(DecodeError::new_err("index out of range")),
            Some(ch) => Ok(*ch),
        }
    }
}
