#![allow(clippy::implicit_return)]
#![allow(clippy::needless_return)]
#![deny(clippy::pedantic)]

mod decode;
mod encode;

use pyo3::prelude::*;

#[pymodule()]
fn _bencode(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(encode::bencode, m)?)?;
    m.add_function(wrap_pyfunction!(decode::bdecode, m)?)?;
    m.add(
        "BencodeEncodeError",
        py.get_type::<encode::BencodeEncodeError>(),
    )?;
    m.add(
        "BencodeDecodeError",
        py.get_type::<decode::BencodeDecodeError>(),
    )?;
    Ok(())
}
