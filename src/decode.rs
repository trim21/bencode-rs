use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use pyo3::{create_exception, PyResult, Python};

create_exception!(bencode2, BencodeDecodeError, pyo3::exceptions::PyException);

#[pyfunction]
pub fn decode<'py>(py: Python<'py>, value: Vec<u8>) -> PyResult<Bound<'py, PyAny>> {
    let _ctx = Decoder {
        // str_key: str_key,
        py: py,
    };

    let result = match _ctx.decode(&mut value.into_iter()) {
        Ok(result) => result,
        Err(e) => return Err(BencodeDecodeError::new_err(e.to_string())),
    };

    return Ok(result.into_bound(py));
}

pub type DecodeResult = Result<PyObject, PyErr>;

struct Decoder<'a> {
    // str_key: bool,
    py: Python<'a>,
}

impl<'a> Decoder<'a> {
    fn decode<T>(&self, bytes: &mut T) -> DecodeResult
    where
        T: Iterator<Item = u8>,
    {
        let result = match bytes.next() {
            None => return Err(BencodeDecodeError::new_err("empty byte sequence")),
            Some(start_byte) => self.handler(bytes, start_byte),
        };

        if result.is_err() {
            return result;
        }

        return match bytes.next() {
            None => result,
            Some(_) => return Err(BencodeDecodeError::new_err("invalid byte sequence")),
        };
    }

    fn handler<T>(&self, bytes: &mut T, start_byte: u8) -> DecodeResult
    where
        T: Iterator<Item = u8>,
    {
        match start_byte {
            b'0'..=b'9' => self.decode_bytes(bytes, start_byte),
            b'i' => self.decode_int(bytes, start_byte),
            b'l' => self.decode_list(bytes, start_byte),
            b'd' => self.decode_dict(bytes, start_byte),
            _ => return Err(BencodeDecodeError::new_err("invalid byte")),
        }
    }

    fn decode_int<T>(&self, bytes: &mut T, _start_byte: u8) -> DecodeResult
    where
        T: Iterator<Item = u8>,
    {
        let mut buff = vec![];
        let mut sign = 1;

        let nxt = match bytes.next() {
            None => return Err(BencodeDecodeError::new_err("invalid int")),
            Some(ch) => ch,
        };

        if nxt == b'-' {
            sign = -1;
        } else if nxt >= b'0' && nxt <= b'9' {
            buff.push(nxt);
        } else {
            return Err(BencodeDecodeError::new_err("invalid int"));
        }

        while let Some(ch) = bytes.next() {
            match ch {
                b'0'..=b'9' => buff.push(ch),
                b'e' => break,
                _ => return Err(BencodeDecodeError::new_err("integer".to_string())),
            }
        }

        if buff.len() > 1 && buff[0] == b'0' {
            return Err(BencodeDecodeError::new_err("invalid leading zero in int"));
        }

        let i = bytes_to_int(buff)?;

        if sign == -1 && i == 0 {
            return Err(BencodeDecodeError::new_err("-0 as int found"));
        }

        return Ok((i * sign).into_py(self.py));
    }

    fn decode_bytes<T>(&self, bytes: &mut T, start_byte: u8) -> DecodeResult
    where
        T: Iterator<Item = u8>,
    {
        let mut len_buff = vec![start_byte];
        let mut str_buff = vec![];

        while let Some(ch) = bytes.next() {
            match ch {
                b'0'..=b'9' => len_buff.push(ch),
                b':' => break,
                _ => return Err(BencodeDecodeError::new_err("byte string".to_string())),
            }
        }

        let len = bytes_to_int(len_buff)?;

        for _ in 0..len {
            match bytes.next() {
                None => return Err(BencodeDecodeError::new_err("invalid bytes length")),
                Some(ch) => str_buff.push(ch),
            }
        }

        let o = PyBytes::new_bound(self.py, &str_buff);

        return Ok(o.into());
    }

    fn decode_list<T>(&self, bytes: &mut T, _start_byte: u8) -> DecodeResult
    where
        T: Iterator<Item = u8>,
    {
        let mut l = vec![];

        loop {
            match bytes.next() {
                None => return Err(BencodeDecodeError::new_err("invalid list")),
                Some(ch) => match ch {
                    b'e' => break,
                    ch => {
                        let item = self.handler(bytes, ch)?;
                        l.push(item);
                    }
                },
            }
        }

        return Ok(l.into_py(self.py));
    }

    fn decode_dict<T>(&self, bytes: &mut T, _start_byte: u8) -> DecodeResult
    where
        T: Iterator<Item = u8>,
    {
        let d = PyDict::new_bound(self.py);

        loop {
            match bytes.next() {
                None => return Err(BencodeDecodeError::new_err("invalid dict")),
                Some(ch) => match ch {
                    b'e' => break,
                    _ => {
                        let key = self.decode_bytes(bytes, ch)?;
                        let value = self.handler(bytes, ch)?;
                        match d.set_item(key, value) {
                            Ok(_) => {}
                            Err(err) => {
                                return Err(BencodeDecodeError::new_err(err.to_string()));
                            }
                        };
                    }
                },
            }
        }

        return Ok(d.into_py(self.py));
    }
}

fn bytes_to_str(bytes: Vec<u8>) -> String {
    return bytes.iter().map(|&b| b as char).collect::<String>();
}

fn bytes_to_int(bytes: Vec<u8>) -> Result<i128, PyErr> {
    let integer_str = bytes_to_str(bytes);

    return match integer_str.parse::<i128>() {
        Err(_) => return Err(BencodeDecodeError::new_err("invalid int")),
        Ok(i) => Ok(i),
    };
}
