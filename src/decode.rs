use pyo3::ffi::PyLong_FromString;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use pyo3::{create_exception, PyResult, Python};

create_exception!(bencode2, DecodeError, pyo3::exceptions::PyException);

#[pyfunction]
pub fn decode(py: Python<'_>, value: Vec<u8>) -> PyResult<Bound<'_, PyAny>> {
    if value.len() == 0 {
        return Err(DecodeError::new_err("empty byte sequence"));
    }

    let mut ctx = Decoder {
        bytes: value,
        index: 0,
        py,
    };

    return Ok(ctx.decode_any()?.into_bound(py));
}

pub type DecodeResult = Result<PyObject, PyErr>;

struct Decoder<'a> {
    // str_key: bool,
    bytes: Vec<u8>,
    index: usize, // any torrent file larger than 4GiB?
    py: Python<'a>,
}

impl<'a> Decoder<'a> {
    fn decode_any(&mut self) -> DecodeResult {
        return match self.current_byte()? {
            b'i' => self.decode_int(),
            b'0'..=b'9' => self.decode_bytes(),
            b'l' => self.decode_list(),
            b'd' => self.decode_dict(),
            _ => return Err(DecodeError::new_err("invalid leading byte")),
        };
    }

    fn decode_int(&mut self) -> DecodeResult {
        let index_e = match self.bytes[self.index..].iter().position(|&b| b == b'e') {
            Some(i) => i,
            None => return Err(DecodeError::new_err("invalid int")),
        };

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

        match self.bytes[self.index] {
            b'-' => {
                if self.bytes[self.index + 1] == b'0' {
                    return Err(DecodeError::new_err(format!(
                        "invalid int, '-0' found at {}",
                        self.index
                    )));
                }
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

        for c_char in self.bytes[self.index..index_e].iter() {
            let c = c_char - b'0';
            if c > 9 {
                return Err(DecodeError::new_err(format!(
                    "invalid int, '{}' found at {}",
                    c_char, self.index
                )));
            }
        }

        if sign < 0 {
            let mut val: i128 = 0;

            for c_char in self.bytes[self.index..index_e].iter() {
                let c = c_char - b'0';
                val = match val.checked_mul(10).and_then(|v| v.checked_add(c as i128)) {
                    Some(v) => v,
                    None => {
                        return self.decode_int_slow(index_e);
                    }
                }
            }

            val = match val.checked_mul(-1) {
                Some(v) => v,
                None => {
                    return self.decode_int_slow(index_e);
                }
            };

            self.index = index_e + 1;
            return Ok(val.into_py(self.py));
        }

        let mut val: u128 = 0;

        for c_char in self.bytes[self.index..index_e].iter() {
            let c = c_char - b'0';
            val = match val.checked_mul(10).and_then(|v| v.checked_add(c as u128)) {
                Some(v) => v,
                None => {
                    return self.decode_int_slow(index_e);
                }
            }
        }

        self.index = index_e + 1;
        return Ok(val.into_py(self.py));
    }

    fn decode_int_slow(&mut self, index_e: usize) -> DecodeResult {
        let s = self.bytes[self.index..index_e].to_vec();

        self.index = index_e + 1;

        let c_str = std::ffi::CString::new(s).unwrap();
        unsafe {
            let ptr = PyLong_FromString(c_str.as_ptr(), std::ptr::null_mut(), 10);

            // panic!("not implemented");
            return Py::from_owned_ptr_or_err(self.py, ptr);
        };
    }

    fn decode_bytes(&self) -> DecodeResult {
        let index_sep = match self.bytes[self.index..].iter().position(|&b| b == b':') {
            Some(i) => i,
            None => {
                return Err(DecodeError::new_err(format!(
                    "invalid bytes, missing length separator: index {}",
                    self.index
                )))
            }
        };

        if self.bytes[self.index] == b'0' && self.index + 1 != index_sep {
            return Err(DecodeError::new_err(format!(
                "invalid bytes length, leading '0' found at index {}",
                self.index
            )));
        }

        let mut len: usize = 0;
        for c in self.bytes[self.index..index_sep].iter() {
            len = len * 10 + (c - b'0') as usize;
        }

        if self.index + len >= self.bytes.len() {
            return Err(DecodeError::new_err(format!(
                "invalid bytes length, index out of range: index {}, len {}",
                self.index, len
            )));
        }

        let str_buff = self.bytes[index_sep + 1..index_sep + 1 + len].to_vec();

        let o = PyBytes::new_bound(self.py, &str_buff);

        return Ok(o.into());
    }

    fn decode_list(&mut self) -> DecodeResult {
        let mut l = Vec::with_capacity(8);

        loop {
            match self.bytes.get(self.index) {
                None => {
                    return Err(DecodeError::new_err("invalid list, overflow".to_string()));
                }
                Some(b'e') => break,
                Some(_) => {
                    l.push(self.decode_any()?);
                }
            }
        }

        return Ok(l.into_py(self.py));
    }

    fn decode_dict(&mut self) -> DecodeResult {
        let d = PyDict::new_bound(self.py);

        loop {
            match self.bytes.get(self.index) {
                None => return Err(DecodeError::new_err("invalid dict")),
                Some(b'e') => break,
                Some(_) => {
                    let key = self.decode_bytes()?;
                    let value = self.decode_any()?;
                    match d.set_item(key, value) {
                        Ok(_) => {}
                        Err(err) => {
                            return Err(DecodeError::new_err(format!(
                                "failed to decode dict, err {}. index {}",
                                err.to_string(),
                                self.index
                            )));
                        }
                    }
                }
            }
        }

        return Ok(d.into_py(self.py));
    }

    fn current_byte(&self) -> Result<u8, PyErr> {
        return match self.bytes.get(self.index) {
            None => {
                return Err(DecodeError::new_err("index out of range"));
            }
            Some(ch) => Ok(*ch),
        };
    }
}
