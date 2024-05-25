use std::borrow::Cow;
use std::collections::HashSet;

use bytes::{BufMut, Bytes, BytesMut};
use pyo3::exceptions::PyValueError;
use pyo3::{
    create_exception,
    exceptions::PyTypeError,
    ffi::Py_TYPE,
    prelude::*,
    types::{PyBool, PyBytes, PyDict, PyInt, PyList, PyString, PyTuple, PyType},
    PyTypeCheck,
};
use smallvec::SmallVec;

create_exception!(bencode2, BencodeEncodeError, pyo3::exceptions::PyException);

struct Context {
    buf: BytesMut,
    seen: HashSet<usize>,
}

#[pyfunction]
pub fn encode<'py>(py: Python<'py>, value: Bound<'py, PyAny>) -> PyResult<Cow<'py, [u8]>> {
    let mut ctx = Context {
        buf: BytesMut::with_capacity(4096),
        seen: HashSet::with_capacity(100),
    };

    encode_any(py, &mut ctx, value)?;

    return Ok(ctx.buf.to_vec().into());
}

fn encode_any<'py>(py: Python<'py>, ctx: &mut Context, value: Bound<'py, PyAny>) -> PyResult<()> {
    match value.clone().downcast_into::<PyString>() {
        Ok(s) => {
            let str = s.to_string();
            let mut buffer = itoa::Buffer::new();
            ctx.buf.put(buffer.format(str.len()).as_bytes());
            ctx.buf.put_u8(b':');
            ctx.buf.put(str.as_bytes());

            return Ok(());
        }
        Err(_) => {}
    }

    match value.clone().downcast_into::<PyBytes>() {
        Ok(bytes) => {
            let mut buffer = itoa::Buffer::new();
            ctx.buf.put(buffer.format(bytes.len()?).as_bytes());
            ctx.buf.put_u8(b':');
            ctx.buf.put(bytes.as_bytes());

            return Ok(());
        }
        Err(_) => {}
    }

    match value.clone().downcast_into::<PyBool>() {
        Ok(bool) => {
            ctx.buf.put_u8(b'i');

            if bool.is_true() {
                ctx.buf.put_u8(b'1');
            } else {
                ctx.buf.put_u8(b'0');
            }

            ctx.buf.put_u8(b'e');
            return Ok(());
        }
        Err(_) => {}
    }

    match value.clone().downcast_into::<PyInt>() {
        Ok(_) => {
            let v: i128 = value.extract()?;
            let mut buffer = itoa::Buffer::new();

            ctx.buf.put_u8(b'i');
            ctx.buf.put(buffer.format(v).as_bytes());
            ctx.buf.put_u8(b'e');
            return Ok(());
        }
        Err(_) => {}
    }

    let ptr = value.clone().into_ptr().cast::<()>() as usize;
    let repr = value.repr().unwrap().to_string();

    if PyList::type_check(&value) || PyTuple::type_check(&value) {
        if ctx.seen.contains(&ptr) {
            return Err(PyValueError::new_err(format!(
                "circular reference found: {repr}"
            )));
        }
        ctx.seen.insert(ptr);
        let v: Vec<Bound<'py, PyAny>> = value.extract()?;

        ctx.buf.put_u8(b'l');
        for x in v {
            encode_any(py, ctx, x)?;
        }
        ctx.buf.put_u8(b'e');

        ctx.seen.remove(&ptr);

        return Ok(());
    }

    if PyDict::type_check(&value) {
        if ctx.seen.contains(&ptr) {
            return Err(PyValueError::new_err(format!(
                "circular reference found: {repr}"
            )));
        }
        ctx.seen.insert(ptr);

        _encode_dict(py, ctx, value.downcast_into()?)?;
        ctx.seen.remove(&ptr);
        return Ok(());
    }

    let typ = value.get_type();
    let name = typ.name()?;

    return Err(PyTypeError::new_err(format!("Unsupported type '{name}'")));
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

fn _encode_dict<'py>(py: Python<'py>, ctx: &mut Context, v: Bound<'py, PyDict>) -> PyResult<()> {
    ctx.buf.put(&b"d"[..]);

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
        _encode_str(key, &mut ctx.buf)?;
        encode_any(py, ctx, value.into_any())?;
    }

    ctx.buf.put(&b"e"[..]);

    return Ok(());
}
