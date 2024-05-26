use std::collections::HashSet;

use bytes::{BufMut, BytesMut};
use pyo3::{create_exception, exceptions::PyTypeError, prelude::*, types::{PyBool, PyBytes, PyDict, PyInt, PyList, PyString, PyTuple}};
use pyo3::exceptions::PyValueError;
use smallvec::SmallVec;
use syncpool::SyncPool;

create_exception!(bencode_rs, BencodeEncodeError, pyo3::exceptions::PyException);

pub const MIB: usize = 1_048_576;

pub fn init() {
    unsafe {
        CONTEXT_POOL.replace(SyncPool::with_builder(Context::initializer));
    }
}

#[pyfunction]
#[pyo3(text_signature = "(v: Any, /)")]
pub fn bencode<'py>(py: Python<'py>, v: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyBytes>> {
    let mut ctx = get_ctx();
    // let mut ctx = Context::initializer();

    encode_any(&mut ctx, py, v)?;

    let r = PyBytes::new_bound(py, ctx.buf.as_ref());

    release_ctx(ctx);

    return Ok(r);
}

type EncodeError = BencodeEncodeError;

static mut CONTEXT_POOL: Option<SyncPool<Context>> = None;

fn get_ctx() -> Box<Context> {
    unsafe {
        CONTEXT_POOL.as_mut().unwrap().get()
    }
}

fn release_ctx(mut ctx: Box<Context>) -> () {
    // do not store large buffers
    // who encode torrent >= 100 MiB normally?
    if ctx.buf.capacity() > 100 * MIB {
        return;
    }
    ctx.buf.clear();
    ctx.seen.clear();
    unsafe {
        CONTEXT_POOL.as_mut().unwrap().put(ctx);
    }
}


struct Context {
    buf: BytesMut,
    seen: HashSet<usize>,
}

impl Context {
    fn initializer() -> Self {
        Self {
            buf: BytesMut::with_capacity(4096),
            seen: HashSet::with_capacity(100),
        }
    }
}

fn encode_any<'py>(ctx: &mut Context, py: Python<'py>, value: Bound<'py, PyAny>) -> PyResult<()> {
    if let Ok(s) = value.downcast::<PyString>() {
        let str = s.to_string();
        let mut buffer = itoa::Buffer::new();
        ctx.buf.put(buffer.format(str.len()).as_bytes());
        ctx.buf.put_u8(b':');
        ctx.buf.put(str.as_bytes());

        return Ok(());
    }

    if let Ok(bytes) = value.downcast::<PyBytes>() {
        let mut buffer = itoa::Buffer::new();
        ctx.buf.put(buffer.format(bytes.len()?).as_bytes());
        ctx.buf.put_u8(b':');
        ctx.buf.put(bytes.as_bytes());

        return Ok(());
    }

    if let Ok(bool) = value.downcast::<PyBool>() {
        ctx.buf.put_u8(b'i');

        if bool.is_true() {
            ctx.buf.put_u8(b'1');
        } else {
            ctx.buf.put_u8(b'0');
        }

        ctx.buf.put_u8(b'e');
        return Ok(());
    }

    if let Ok(int) = value.downcast::<PyInt>() {
        let v: i128 = int.extract()?;
        let mut buffer = itoa::Buffer::new();

        ctx.buf.put_u8(b'i');
        ctx.buf.put(buffer.format(v).as_bytes());
        ctx.buf.put_u8(b'e');
        return Ok(());
    }

    let ptr = value.as_ptr().cast::<()>() as usize;

    if let Ok(seq) = value.downcast::<PyList>() {
        if ctx.seen.contains(&ptr) {
            let repr = value.repr().unwrap().to_string();
            return Err(PyValueError::new_err(
                format!("circular reference found {repr}")
            ));
        }

        ctx.seen.insert(ptr);
        ctx.buf.put_u8(b'l');

        for x in seq {
            encode_any(ctx, py, x)?;
        }

        ctx.buf.put_u8(b'e');
        ctx.seen.remove(&ptr);

        return Ok(());
    }

    if let Ok(seq) = value.downcast::<PyTuple>() {
        if ctx.seen.contains(&ptr) {
            let repr = value.repr().unwrap().to_string();
            return Err(PyValueError::new_err(
                format!("circular reference found {repr}")
            ));
        }

        ctx.seen.insert(ptr);
        ctx.buf.put_u8(b'l');

        for x in seq {
            encode_any(ctx, py, x)?;
        }

        ctx.buf.put_u8(b'e');
        ctx.seen.remove(&ptr);

        return Ok(());
    }


    if let Ok(dict) = value.downcast::<PyDict>() {
        if ctx.seen.contains(&ptr) {
            let repr = value.repr().unwrap().to_string();
            return Err(PyValueError::new_err(format!(
                "circular reference found: {repr}"
            )));
        }
        ctx.seen.insert(ptr);

        encode_dict(ctx, py, dict)?;

        ctx.seen.remove(&ptr);
        return Ok(());
    }

    let typ = value.get_type();
    let name = typ.name()?;

    return Err(PyTypeError::new_err(format!("Unsupported type '{name}'")));
}

#[inline]
fn _encode_str<'py>(v: String, buf: &mut BytesMut) -> PyResult<()> {
    let mut buffer = itoa::Buffer::new();

    buf.put(buffer.format(v.len()).as_bytes());
    buf.put_u8(b':');
    buf.put(v.as_bytes());

    return Ok(());
}

fn encode_dict<'py>(ctx: &mut Context, py: Python<'py>, v: &Bound<'py, PyDict>) -> PyResult<()> {
    ctx.buf.put_u8(b'd');

    let mut sv: SmallVec<[(String, Bound<'_, PyAny>); 8]> = SmallVec::with_capacity(v.len());

    for item in v.items().iter() {
        let (key, value): (PyObject, Bound<'_, PyAny>) = item.extract()?;

        if let Ok(d) = key.extract::<&PyString>(py) {
            let bb = d.to_string();
            sv.push((bb, value));
            continue;
        }

        if let Ok(d) = key.extract::<&PyBytes>(py) {
            sv.push((String::from_utf8(d.as_bytes().into())?, value));
            continue;
        }

        let typ = value.get_type();
        let name = typ.name()?;

        return Err(PyTypeError::new_err(format!("Unsupported type '{name}' as dict key")));
    }

    sv.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    let mut last_key: Option<String> = None;
    for (key, _) in sv.clone() {
        if let Some(lk) = last_key {
            if lk == key {
                return Err(EncodeError::new_err(format!(
                    "Duplicated keys {key}"
                )));
            }
        }

        last_key = Some(key);
    }

    for (key, value) in sv {
        _encode_str(key, &mut ctx.buf)?;
        encode_any(ctx, py, value.into_any())?;
    }

    ctx.buf.put_u8(b'e');

    return Ok(());
}
