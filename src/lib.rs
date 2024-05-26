#![deny(clippy::implicit_return)]
#![deny(clippy::needless_return)]

mod decode;
mod encode;

use pyo3::prelude::*;

#[pymodule]
fn _bencode(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    encode::init();

    m.add_function(wrap_pyfunction!(encode::bencode, m)?)?;
    m.add_function(wrap_pyfunction!(decode::bdecode, m)?)?;
    m.add(
        "BencodeEncodeError",
        py.get_type_bound::<encode::BencodeEncodeError>(),
    )?;
    m.add(
        "BencodeDecodeError",
        py.get_type_bound::<decode::BencodeDecodeError>(),
    )?;
    return Ok(());
}
