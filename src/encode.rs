use std::borrow::Cow;

use bytes::{BufMut, Bytes, BytesMut};
use pyo3::{
    create_exception,
    exceptions::{PyException, PyTypeError},
    ffi::{self, Py_TYPE},
    prelude::*,
    types::{PyBool, PyBytes, PyDict, PyInt, PyList, PyString, PyTuple, PyType},
    PyTypeCheck,
};
use smallvec::SmallVec;

create_exception!(bencode2, BencodeEncodeError, pyo3::exceptions::PyException);

// pub fn encode(value: PyObject, py: Python<'_>) -> PyResult<Cow<'_, [u8]>> {
#[pyfunction]
pub fn encode<'py>(py: Python<'py>, value: Bound<'py, PyAny>) -> PyResult<Cow<'py, [u8]>> {
    let mut buf: BytesMut = BytesMut::with_capacity(4096);

    _encode(py, &mut buf, value)?;

    return Ok(buf.to_vec().into());
}

pub fn _encode<'py>(py: Python<'py>, buf: &mut BytesMut, value: Bound<'py, PyAny>) -> PyResult<()> {
    if PyString::type_check(&value) {
        return _encode_str(value.extract()?, buf);
    }

    if PyBytes::type_check(&value) {
        let v: Cow<[u8]> = value.extract()?;
        // buf.put(&b"[bytes]"[..]);
        return _encode_bytes(v, buf);
    }

    if PyBool::type_check(&value) {
        buf.put_u8(b'i');

        unsafe {
            if value.as_ptr() == ffi::Py_True() {
                buf.put_u8(b'1');
            } else {
                buf.put_u8(b'0');
            }
        }

        // TODO: us `v is True` instead of `bool(v)`
        // if value.is_truthy()? {
        //     buf.put_u8(b'1');
        // } else {
        //     buf.put_u8(b'0');
        // }

        buf.put_u8(b'e');
        return Ok(());
    }

    if PyInt::type_check(&value) {
        let v: i128 = value.extract()?;
        let mut buffer = itoa::Buffer::new();

        buf.put_u8(b'i');
        buf.put(buffer.format(v).as_bytes());
        buf.put_u8(b'e');
        return Ok(());
    }

    if PyList::type_check(&value) || PyTuple::type_check(&value) {
        let v: Vec<Bound<'py, PyAny>> = value.extract()?;

        buf.put_u8(b'l');
        for x in v {
            _encode(py, buf, x)?;
        }
        buf.put_u8(b'e');
        return Ok(());
    }

    if PyDict::type_check(&value) {
        if let Ok(d) = value.extract::<Bound<'py, PyDict>>() {
            return _encode_dict(py, buf, d);
        } else {
            return Err(PyException::new_err(
                "unexpected error, failed to extract dict".to_string(),
            ));
        }
    }

    // unsafe {
    //     if PyLong_Check(o) == 1 {
    //         let mut res: i32 = 0;
    //         let v = PyLong_AsLongLongAndOverflow(o, &res);
    //         if res == 0 {
    //             return _encode_int(v, buf);
    //         }

    //         let err = PyErr_Occurred();
    //         if res == -1 && err != std::ptr::null_mut() {
    //             return Err(PyException::new_err(
    //                 "Failed to convert long to object".to_string(),
    //             ));
    //         }

    //         PyErr_Clear();
    //         let o = PyString::new_bound(py, "%d");

    //         return _encode_int_slow(buf, value);
    //     }

    //     if PyBool_Check(o) == 1 {
    //         buf.put(&b"bool"[..]);
    //         buf.put(&b"i"[..]);

    //         if value.is_truthy(py)? {
    //             buf.put(&b"1"[..]);
    //         } else {
    //             buf.put(&b"0"[..]);
    //         }

    //         buf.put(&b"e"[..]);
    //         return Ok(());
    //     }

    //     if (PyTuple_Check(o) == 1) {
    //         let v: Vec<PyObject> = value.extract(py)?;
    //         buf.put(&b"l"[..]);
    //         for x in v {
    //             _encode(x, py, buf)?;
    //         }
    //         buf.put(&b"e"[..]);
    //         return Ok(());
    //     }

    //     if (PyList_Check(o) == 1) || (PyTuple_Check(o) == 1) {
    //         let v: Vec<PyObject> = value.extract(py)?;
    //         buf.put(&b"l"[..]);
    //         for x in v {
    //             _encode(x, py, buf)?;
    //         }
    //         buf.put(&b"e"[..]);
    //         return Ok(());
    //     }

    //     if PyUnicode_Check(o) == 1 {
    //         return _encode_str(value.extract(py)?, buf);
    //     }

    //     if PyBytes_Check(o) == 1 {
    //         let v: Cow<[u8]> = value.extract(py)?;
    //         // buf.put(&b"[bytes]"[..]);
    //         return _encode_bytes(v, buf);
    //     }
    // }

    // PyType::new_bound(py, &value)).qualname()?;

    // return Err(BencodeEncodeError::new_err("Unsupported type".to_string()));

    unsafe {
        let typ = Py_TYPE(value.as_ptr());

        let bb = PyType::from_borrowed_type_ptr(py, typ);
        let name = bb.name()?;

        return Err(PyTypeError::new_err(format!("Unsupported type '{name}'")));
    }
}

fn _encode_bytes(v: Cow<[u8]>, buf: &mut BytesMut) -> PyResult<()> {
    let mut buffer = itoa::Buffer::new();

    buf.put(buffer.format(v.len()).as_bytes());
    buf.put_u8(b':');
    buf.put(Bytes::from(v.to_vec()));

    return Ok(());
}

#[inline]
fn _encode_str<'py>(v: String, buf: &mut BytesMut) -> PyResult<()> {
    let mut buffer = itoa::Buffer::new();

    buf.put(buffer.format(v.len()).as_bytes());
    buf.put_u8(b':');
    buf.put(v.as_bytes());

    return Ok(());
}

fn _encode_dict<'py>(py: Python<'py>, buf: &mut BytesMut, v: Bound<'py, PyDict>) -> PyResult<()> {
    buf.put(&b"d"[..]);

    let mut sv: SmallVec<[(String, Bound<'_, PyAny>); 8]> = SmallVec::with_capacity(v.len());

    for item in v.items().iter() {
        let (key, value): (PyObject, Bound<'_, PyAny>) = item.extract()?;

        if let Ok(d) = key.extract::<&PyString>(py) {
            let bb = d.to_string();
            sv.push((bb, value));
        } else if let Ok(d) = key.extract::<&PyBytes>(py) {
            sv.push((String::from_utf8(d.as_bytes().into())?, value));
        } else {
            unsafe {
                let typ = Py_TYPE(key.as_ptr());

                let bb = PyType::from_borrowed_type_ptr(py, typ);
                let name = bb.qualname()?;

                return Err(PyTypeError::new_err(format!(
                    "Unsupported type {name} as dict keys"
                )));
            }
        }
    }

    sv.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    let mut last_key: Option<String> = None;
    for (key, _) in sv.clone() {
        if let Some(lk) = last_key {
            if lk == key {
                return Err(BencodeEncodeError::new_err(format!(
                    "Duplicated keys {key}"
                )));
            }
        }

        last_key = Some(key);
    }

    for (key, value) in sv {
        _encode_str(key, buf)?;
        _encode(py, buf, value.into_any())?;
    }

    buf.put(&b"e"[..]);

    return Ok(());
}
