use bytes::BufMut;
use pyo3::types::PyByteArray;
use pyo3::{
    create_exception,
    exceptions::PyTypeError,
    prelude::*,
    types::{PyBytes, PyDict, PyInt, PyList, PyString, PyTuple},
};
use pyo3::{ffi, PyTypeCheck};
use smallvec::SmallVec;
use std::borrow::Cow;
use std::collections::HashSet;
use std::io::Write;
use std::sync::{LazyLock, Mutex};

create_exception!(
    bencode_rs,
    BencodeEncodeError,
    pyo3::exceptions::PyException
);

pub const MIB: usize = 1024 * 1024;

#[pyfunction]
#[pyo3(text_signature = "(v: Any, /)")]
pub fn bencode<'py>(py: Python<'py>, v: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyBytes>> {
    let mut ctx = get_ctx();

    let result = encode_any(&mut ctx, py, v);

    return match result {
        Ok(()) => {
            let rr = PyBytes::new(py, ctx.buf.as_ref());
            release_ctx(ctx);
            Ok(rr)
        }
        Err(err) => {
            release_ctx(ctx);
            Err(err)
        }
    };
}

type EncodeError = BencodeEncodeError;

static CONTEXT_POOL: LazyLock<Mutex<Vec<Context>>> =
    LazyLock::new(|| Mutex::new(Vec::with_capacity(4)));

fn get_ctx() -> Context {
    let mut pool = CONTEXT_POOL.lock().unwrap();

    if let Some(ctx) = pool.pop() {
        return ctx;
    }

    return Context::default();
}

fn release_ctx(mut ctx: Context) {
    if ctx.buf.capacity() > MIB {
        return;
    }

    let mut pool = CONTEXT_POOL.lock().unwrap();
    if pool.len() < 4 {
        ctx.buf.clear();
        ctx.seen.clear();
        ctx.stack_depth = 0;
        pool.push(ctx);
    }
}

struct Context {
    buf: Vec<u8>,
    seen: HashSet<usize>,
    stack_depth: usize,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            buf: Vec::with_capacity(4096),
            seen: HashSet::with_capacity(100),
            stack_depth: 0,
        }
    }
}

impl Context {
    fn write_int<Int: num::Integer + std::fmt::Display + Copy>(
        self: &mut Context,
        val: Int,
    ) -> std::io::Result<()> {
        std::write!(&mut self.buf, "{val}")?;
        Ok(())
    }
}

fn encode_any<'py>(ctx: &mut Context, py: Python<'py>, value: &Bound<'py, PyAny>) -> PyResult<()> {
    if PyString::type_check(value) {
        let s = unsafe { value.cast_unchecked::<PyString>() };
        let b = s.to_str()?;
        ctx.write_int(b.len())?;
        ctx.buf.put_u8(b':');
        ctx.buf.put(b.as_bytes());
        return Ok(());
    }

    if PyBytes::type_check(value) {
        let bytes = unsafe { value.cast_unchecked::<PyBytes>() };

        let b = bytes.as_bytes();

        ctx.write_int(b.len())?;
        ctx.buf.put_u8(b':');
        ctx.buf.put(b);

        return Ok(());
    }

    if PyInt::type_check(value) {
        return encode_int(ctx, py, value);
    }

    let ptr = value.as_ptr().cast::<()>() as usize;

    if PyDict::type_check(value) {
        ctx.stack_depth += 1;
        let checked = ctx.stack_depth >= 100;

        if checked {
            if ctx.seen.contains(&ptr) {
                let repr = value.repr()?.to_string();
                return Err(BencodeEncodeError::new_err(format!(
                    "circular reference found: {repr}"
                )));
            }
            ctx.seen.insert(ptr);
        }

        unsafe {
            encode_dict(ctx, py, value.cast_unchecked())?;
        }

        if checked {
            ctx.seen.remove(&ptr);
        }

        return Ok(());
    }

    if PyList::type_check(value) {
        ctx.stack_depth += 1;
        let checked = ctx.stack_depth >= 100;

        if checked {
            if ctx.seen.contains(&ptr) {
                let repr = value.repr()?.to_string();
                return Err(BencodeEncodeError::new_err(format!(
                    "circular reference found: {repr}"
                )));
            }
            ctx.seen.insert(ptr);
        }

        ctx.buf.put_u8(b'l');

        let seq = unsafe { value.cast_unchecked::<PyList>() };

        for x in seq.iter() {
            encode_any(ctx, py, &x)?;
        }

        ctx.buf.put_u8(b'e');

        if checked {
            ctx.seen.remove(&ptr);
        }

        return Ok(());
    }

    if PyTuple::type_check(value) {
        ctx.stack_depth += 1;
        let checked = ctx.stack_depth >= 100;

        if checked {
            if ctx.seen.contains(&ptr) {
                let repr = value.repr()?.to_string();
                return Err(BencodeEncodeError::new_err(format!(
                    "circular reference found: {repr}"
                )));
            }
            ctx.seen.insert(ptr);
        }

        ctx.buf.put_u8(b'l');

        let seq = unsafe { value.cast_unchecked::<PyTuple>() };

        for x in seq.iter() {
            encode_any(ctx, py, &x)?;
        }

        ctx.buf.put_u8(b'e');

        if checked {
            ctx.seen.remove(&ptr);
        }

        return Ok(());
    }

    if PyByteArray::type_check(value) {
        let bytes = unsafe { value.cast_unchecked::<PyByteArray>() };

        let b = unsafe { bytes.as_bytes() };

        ctx.write_int(b.len())?;
        ctx.buf.put_u8(b':');
        ctx.buf.put(b);

        return Ok(());
    }

    let typ = value.get_type();
    let name = typ.name()?;

    Err(PyTypeError::new_err(format!("Unsupported type '{name}'")))
}

#[inline]
fn __encode_str(v: &[u8], ctx: &mut Context) -> PyResult<()> {
    ctx.write_int(v.len())?;
    ctx.buf.put_u8(b':');
    ctx.buf.put(v.as_ref());

    Ok(())
}

struct AutoFree {
    pub ptr: *mut ffi::PyObject,
}

impl Drop for AutoFree {
    fn drop(&mut self) {
        unsafe {
            ffi::Py_DecRef(self.ptr);
        }
    }
}

fn encode_int<'py>(ctx: &mut Context, py: Python<'py>, value: &Bound<'py, PyAny>) -> PyResult<()> {
    let v = unsafe { value.cast_unchecked::<PyInt>() };

    if let Ok(v) = v.extract::<i64>() {
        ctx.buf.put_u8(b'i');
        ctx.write_int(v)?;
        ctx.buf.put_u8(b'e');

        return Ok(());
    }

    ctx.buf.put_u8(b'i');

    unsafe {
        let i = ffi::PyNumber_Long(value.as_ptr());
        if i.is_null() {
            return Err(PyErr::fetch(py));
        }

        let o = AutoFree { ptr: i };
        let s = ffi::PyObject_Str(o.ptr);
        if s.is_null() {
            return Err(PyErr::fetch(py));
        }

        let ss = Py::<PyAny>::from_owned_ptr(py, s);

        let s = ss.cast_bound_unchecked::<PyString>(py);
        ctx.buf.put(s.to_str()?.as_bytes());
    };

    ctx.buf.put_u8(b'e');

    Ok(())
}

fn encode_dict<'py>(ctx: &mut Context, py: Python<'py>, v: &Bound<'py, PyDict>) -> PyResult<()> {
    ctx.buf.put_u8(b'd');

    #[allow(clippy::type_complexity)]
    let mut sv: SmallVec<[(Cow<[u8]>, Bound<'_, PyAny>); 8]> = SmallVec::with_capacity(v.len());

    for (key, value) in v.iter() {
        if let Ok(s) = key.extract::<&str>() {
            unsafe {
                // d.as_bytes() return a &[u8] and doesn't live longer than variable `key`,
                // but it's not true, &[u8] lives as long as python ptr lives,
                // which is longer than variable `key` and we do not need to drop it.
                sv.push((
                    Cow::from(std::mem::transmute::<&[u8], &'py [u8]>(s.as_bytes())),
                    value,
                ));
            }
            continue;
        }

        if let Ok(b) = key.cast::<PyBytes>() {
            unsafe {
                // d.as_bytes() return a &[u8] and doesn't live longer than variable `key`,
                // but it's not true, &[u8] lives as long as python ptr lives,
                // which is longer than variable `key` and we do not need to drop it.
                sv.push((
                    Cow::from(std::mem::transmute::<&[u8], &'py [u8]>(b.as_bytes())),
                    value,
                ));
            }
            continue;
        }

        let typ = value.get_type();
        let name = typ.name()?;

        return Err(PyTypeError::new_err(format!(
            "Unsupported type '{name}' as dict key"
        )));
    }

    sv.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    let mut last_key: Option<Cow<[u8]>> = None;
    for (key, _) in sv.clone() {
        if let Some(lk) = last_key {
            if lk == key {
                return Err(EncodeError::new_err(format!(
                    "Duplicated keys {}",
                    String::from_utf8(lk.into())?
                )));
            }
        }

        last_key = Some(key);
    }

    for (key, value) in sv {
        __encode_str(&key, ctx)?;
        encode_any(ctx, py, &value.into_any())?;
    }

    ctx.buf.put_u8(b'e');

    Ok(())
}
