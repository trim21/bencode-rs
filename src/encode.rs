use bytes::{BufMut, BytesMut};
use once_cell::sync::Lazy;
use pyo3::exceptions::PyValueError;
use pyo3::PyTypeCheck;
use pyo3::{
    create_exception,
    exceptions::PyTypeError,
    prelude::*,
    types::{PyBytes, PyDict, PyInt, PyList, PyString, PyTuple},
};
use smallvec::SmallVec;
use std::borrow::Cow;
use std::collections::HashSet;
use std::io::Write;
use syncpool::SyncPool;

create_exception!(
    bencode_rs,
    BencodeEncodeError,
    pyo3::exceptions::PyException
);

pub const MIB: usize = 1_048_576;

#[pyfunction]
#[pyo3(text_signature = "(v: Any, /)")]
pub fn bencode<'py>(py: Python<'py>, v: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyBytes>> {
    let mut ctx = get_ctx();
    // let mut ctx = Context::default();
    // let mut ctx = Context::initializer();

    encode_any(&mut ctx, py, v)?;

    let r = PyBytes::new(py, ctx.buf.as_ref());

    release_ctx(ctx);

    Ok(r)
}

type EncodeError = BencodeEncodeError;

static mut CONTEXT_POOL: Lazy<SyncPool<Context>> = Lazy::new(SyncPool::new);

fn get_ctx() -> Context {
    unsafe { *CONTEXT_POOL.get() }
}

fn release_ctx(mut ctx: Context) {
    if ctx.buf.capacity() > 100 * MIB {
        return;
    }
    ctx.buf.clear();
    ctx.seen.clear();
    ctx.stack_depth = 0;
    unsafe {
        CONTEXT_POOL.put(Box::from(ctx));
    }
}

struct Context {
    buf: BytesMut,
    seen: HashSet<usize>,
    stack_depth: usize,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            buf: BytesMut::with_capacity(4096),
            seen: HashSet::with_capacity(100),
            stack_depth: 0,
        }
    }
}

impl Context {
    fn write_int<Int: num::Integer + std::fmt::Display>(
        self: &mut Context,
        val: Int,
    ) -> std::io::Result<()> {
        std::write!((&mut self.buf).writer(), "{val}")?;
        Ok(())
    }
}

fn encode_any<'py>(ctx: &mut Context, py: Python<'py>, value: &Bound<'py, PyAny>) -> PyResult<()> {
    if PyString::type_check(value) {
        let s = unsafe { value.downcast_unchecked::<PyString>() };
        let b = s.to_str()?;
        ctx.write_int(b.len())?;
        ctx.buf.put_u8(b':');
        ctx.buf.put(b.as_bytes());
        return Ok(());
    }

    if PyBytes::type_check(value) {
        let bytes = unsafe { value.downcast_unchecked::<PyBytes>() };

        let b = bytes.as_bytes();

        ctx.write_int(b.len())?;
        ctx.buf.put_u8(b':');
        ctx.buf.put(b);

        return Ok(());
    }

    if PyInt::type_check(value) {
        let v = unsafe { value.downcast_unchecked::<PyInt>() };

        match v.extract::<i64>() {
            Ok(v) => {
                ctx.buf.put_u8(b'i');
                ctx.write_int(v)?;
                ctx.buf.put_u8(b'e');

                return Ok(());
            }
            Err(_) => todo!(),
        }
    }

    let ptr = value.as_ptr().cast::<()>() as usize;

    if PyDict::type_check(value) {
        ctx.stack_depth += 1;
        let checked = ctx.stack_depth >= 1000;

        if checked {
            if ctx.seen.contains(&ptr) {
                let repr = value.repr()?.to_string();
                return Err(PyValueError::new_err(format!(
                    "circular reference found: {repr}"
                )));
            }
            ctx.seen.insert(ptr);
        }

        unsafe {
            encode_dict(ctx, py, value.downcast_unchecked())?;
        }

        if checked {
            ctx.seen.remove(&ptr);
        }

        return Ok(());
    }

    if PyList::type_check(value) {
        ctx.stack_depth += 1;
        let checked = ctx.stack_depth >= 1000;

        if checked {
            if ctx.seen.contains(&ptr) {
                let repr = value.repr()?.to_string();
                return Err(PyValueError::new_err(format!(
                    "circular reference found: {repr}"
                )));
            }
            ctx.seen.insert(ptr);
        }

        ctx.buf.put_u8(b'l');

        let seq = unsafe { value.downcast_unchecked::<PyList>() };

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
        let checked = ctx.stack_depth >= 1000;

        if checked {
            if ctx.seen.contains(&ptr) {
                let repr = value.repr()?.to_string();
                return Err(PyValueError::new_err(format!(
                    "circular reference found: {repr}"
                )));
            }
            ctx.seen.insert(ptr);
        }

        ctx.buf.put_u8(b'l');

        let seq = unsafe { value.downcast_unchecked::<PyTuple>() };

        for x in seq.iter() {
            encode_any(ctx, py, &x)?;
        }

        ctx.buf.put_u8(b'e');

        if checked {
            ctx.seen.remove(&ptr);
        }

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

        if let Ok(b) = key.downcast::<PyBytes>() {
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
