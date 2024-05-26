use std::borrow::Cow;
use std::collections::HashMap;
use pyo3::ffi::{PyLong_FromString};
use pyo3::prelude::*;
use pyo3::{create_exception, PyResult, Python};
use pyo3::exceptions::PyOverflowError;
use pyo3::types::{PyBytes, PyDict, PyLong};

create_exception!(bencode_rs,BencodeDecodeError, pyo3::exceptions::PyException);

type DecodeError = BencodeDecodeError;

#[pyfunction]
pub fn bdecode<'py>(py: Python<'py>, b: Bound<'py, PyBytes>) -> PyResult<Bound<'py, PyAny>> {
    if b.len().unwrap() == 0 {
        return Err(DecodeError::new_err("empty byte sequence"));
    }

    let mut ctx = Decoder {
        bytes: b.as_bytes().to_vec(),
        index: 0,
        py,
        depth: 0,
    };

    return Ok(ctx.decode_any()?.into_bound(py));
}


pub type Object = PyObject;
pub type DecodeResult = Result<Object, PyErr>;

struct Decoder<'a> {
    // str_key: bool,
    bytes: Vec<u8>,
    index: usize, // any torrent file larger than 4GiB?
    py: Python<'a>,
    depth: usize,
}

impl<'a> Decoder<'a> {
    fn decode_any(&mut self) -> Result<PyObject, PyErr> {
        return match self.current_byte()? {
            b'i' => self.decode_int(),
            b'0'..=b'9' => Ok(Cow::from(self.decode_bytes()?).into_py(self.py)),
            b'l' => self.decode_list(),
            b'd' => self.decode_dict(),
            _ => return Err(DecodeError::new_err("invalid leading byte")),
        };
    }

    fn decode_int(&mut self) -> Result<PyObject, PyErr> {
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
                        "invalid int, '-0' found at {}", self.index
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

        for c in self.bytes[num_start..index_e].iter() {
            if !(b'0' <= *c && *c <= b'9') {
                return Err(DecodeError::new_err(
                    format!("invalid int, '{}' found at {}", *c as char, self.index)
                ));
            }
        }

        if sign < 0 {
            // slow path to build PyLong with python
            return self.decode_int_slow(index_e);
        }

        let mut val: u128 = 0;

        for c_char in self.bytes[num_start..index_e].iter() {
            let c = *c_char - b'0';
            val = match val.checked_mul(10).and_then(|v| v.checked_add(c as u128)) {
                Some(v) => v,
                None => {
                    return self.decode_int_slow(index_e);
                }
            }
        }

        self.index = index_e + 1;
        return Ok(val.into_py(self.py).into());
    }

    fn decode_int_slow(&mut self, index_e: usize) -> Result<PyObject, PyErr> {
        let s = self.bytes[self.index..index_e].to_vec();

        self.index = index_e + 1;

        let c_str = std::ffi::CString::new(s).unwrap();
        unsafe {
            let ptr = PyLong_FromString(c_str.as_ptr(), std::ptr::null_mut(), 10);

            // panic!("not implemented");
            return Py::from_owned_ptr_or_err(self.py, ptr);
        };
    }

    fn decode_bytes(&mut self) -> Result<Vec<u8>, PyErr> {
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
        for c in self.bytes[self.index..index_sep].iter() {
            len = len * 10 + (c - b'0') as usize;
        }

        if index_sep + len >= self.bytes.len() {
            return Err(DecodeError::new_err(format!(
                "invalid bytes length, index out of range: index {}, len {}",
                self.index, len
            )));
        }

        let str_buff = self.bytes[index_sep + 1..index_sep + 1 + len].to_vec();

        self.index = index_sep + len + 1;

        // let o = PyBytes::new_bound(self.py, &str_buff);

        return Ok(str_buff);
    }

    // fn check_depth(&mut self) -> Result<(), PyErr> {
    //     if self.depth > 10000 {
    //         return Err(DecodeError::new_err("object depth limit 10000 reached"));
    //     }
    //
    //     return Ok(());
    // }

    fn decode_list(&mut self) -> DecodeResult {
        // self.depth += 1;
        // self.check_depth()?;

        self.index += 1;
        let mut l = Vec::with_capacity(8);

        loop {
            match self.bytes.get(self.index) {
                None => {
                    return Err(DecodeError::new_err("unexpected end when parsing list".to_string()));
                }
                Some(b'e') => break,
                Some(_) => {
                    l.push(self.decode_any()?);
                }
            }
        }

        // self.depth -= 1;
        self.index += 1;
        return Ok(l.into_py(self.py));
    }

    fn decode_dict(&mut self) -> DecodeResult {
        // self.depth += 1;
        // self.check_depth()?;
        self.index += 1;

        let mut map: HashMap<Cow<[u8]>, Object> = HashMap::with_capacity(8);
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
                    let ck = Cow::from(key.clone());
                    if let Some(lk) = last_key {
                        if lk > ck {
                            return Err(DecodeError::new_err(format!("dict key not sorted. index {}", self.index)));
                        } else if lk == ck {
                            return Err(DecodeError::new_err(format!("duplicated dict key found: index {}", self.index)));
                        }
                    }
                    map.insert(ck.clone(), value);
                    last_key = Some(ck);
                }
            }
        }

        let d = PyDict::new_bound(self.py);

        for (k, v) in map.iter_mut() {
            d.set_item(k.to_owned().into_py(self.py), v.to_owned())?;
        }

        // self.depth -= 1;
        self.index += 1;
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
