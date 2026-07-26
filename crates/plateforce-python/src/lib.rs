//! Spike. Replaced by the real binding once the pyo3 surface is confirmed.

use pyo3::buffer::PyBuffer;
use pyo3::prelude::*;

pyo3::create_exception!(plateforce, SpikeError, pyo3::exceptions::PyException, "spike");

#[pyclass(frozen)]
struct Spike {
    #[pyo3(get)]
    value: f64,
}

#[pymethods]
impl Spike {
    #[new]
    #[pyo3(signature = (value = 1.0))]
    fn new(value: f64) -> Self {
        Spike { value }
    }

    #[getter]
    fn doubled(&self) -> f64 {
        self.value * 2.0
    }

    fn __repr__(&self) -> String {
        format!("Spike(value={})", self.value)
    }
}

#[pyfunction]
fn sum_buffer(py: Python<'_>, values: &Bound<'_, PyAny>) -> PyResult<f64> {
    let buffer = PyBuffer::<f64>::get(values)?;
    let mut out = vec![0.0f64; buffer.item_count()];
    buffer.copy_to_slice(py, &mut out)?;
    Ok(out.iter().sum())
}

#[pymodule]
fn plateforce(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Spike>()?;
    m.add_function(wrap_pyfunction!(sum_buffer, m)?)?;
    m.add("SpikeError", m.py().get_type::<SpikeError>())?;
    Ok(())
}
