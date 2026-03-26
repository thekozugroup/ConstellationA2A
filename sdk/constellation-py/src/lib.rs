use pyo3::prelude::*;

/// Python module for Constellation A2A SDK.
/// Full bindings will be implemented in a future iteration.
#[pymodule]
fn constellation(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", "0.1.0")?;
    Ok(())
}
