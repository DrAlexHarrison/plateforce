//! Saved plates from a notebook: naming one this machine holds, and stating one it does not.
//!
//! The store is `plateforce_core::plate_store`, the same one the terminal reads, so a plate
//! saved with `plateforce plate save` is the plate `Plate.saved("lab-kistler-1")` reaches and
//! the revision a result attributes to it is the same string on both surfaces.
//!
//! A notebook that holds the members and no file states them with `Plate(name, acquisition)`,
//! which is how a request that travels between machines carries a plate the receiving machine
//! never saved.

use std::path::PathBuf;

use plateforce_core::plate_store;
use plateforce_core::SavedPlate;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::errors::raise_refusal;
use crate::trial::Acquisition;

/// A plate, under the name it is filed or stated under.
#[pyclass(frozen, from_py_object, module = "plateforce", name = "Plate")]
#[derive(Clone)]
pub struct Plate {
    pub(crate) inner: SavedPlate,
}

#[pymethods]
impl Plate {
    /// A plate from members the caller holds, with no file behind it.
    ///
    /// `revision` is computed here from the members and never accepted from the caller: it is
    /// the one thing that tells two revisions of a plate apart, and a caller that could set it
    /// could say two different plates were one.
    #[new]
    #[pyo3(signature = (name, acquisition))]
    fn new(python: Python<'_>, name: &str, acquisition: &Acquisition) -> PyResult<Self> {
        SavedPlate::named(name, acquisition.block())
            .map(|inner| Self { inner })
            .map_err(|refusal| raise_refusal(python, &refusal))
    }

    /// The plate this machine has saved under that name.
    #[staticmethod]
    #[pyo3(signature = (name, plates_directory = None))]
    fn saved(python: Python<'_>, name: &str, plates_directory: Option<PathBuf>) -> PyResult<Self> {
        plate_store::read(name, plates_directory.as_deref())
            .map(|inner| Self { inner })
            .map_err(|refusal| raise_refusal(python, &refusal))
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// Digest over the members this plate holds. Two labs that recorded the same answers hold
    /// one revision whatever they call the plate, and an edit to any member is a different one.
    #[getter]
    fn revision(&self) -> &str {
        &self.inner.revision
    }

    #[getter]
    fn acquisition(&self) -> Acquisition {
        Acquisition::of(&self.inner.members)
    }

    /// Where this plate is filed, and None for one stated from its members.
    #[getter]
    fn path(&self) -> Option<String> {
        self.inner
            .path
            .as_deref()
            .map(|path| path.display().to_string())
    }

    /// True only when every member is present. A run filled from a plate short of one reports
    /// a block short of it, and fingerprints as incomplete rather than as matching.
    #[getter]
    fn is_complete(&self) -> bool {
        self.inner.members.is_complete()
    }

    #[getter]
    fn missing(&self) -> Vec<&'static str> {
        self.inner.members.missing()
    }

    fn __repr__(&self) -> String {
        format!(
            "Plate(name='{}', revision='{}', is_complete={}, missing={:?})",
            self.inner.name,
            self.inner.revision,
            if self.is_complete() { "True" } else { "False" },
            self.missing()
        )
    }
}

/// One plate written to this machine, and the plate that was there before it.
///
/// The previous one travels beside the new one rather than being passed over, because saving
/// over a name is the edit that leaves an already-recorded result resting on answers this
/// machine no longer holds, and a caller reads it where they are already looking.
#[pyclass(
    frozen,
    skip_from_py_object,
    module = "plateforce",
    name = "PlateSaved"
)]
pub struct PlateSaved {
    #[pyo3(get)]
    plate: Plate,
    #[pyo3(get)]
    replaced: Option<Plate>,
    replaced_members: Vec<(String, String, String)>,
}

#[pymethods]
impl PlateSaved {
    /// Each member whose answer moved, as `member: (was, now)`.
    #[getter]
    fn replaced_members<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let moved = PyDict::new(python);
        for (member, was, now) in &self.replaced_members {
            moved.set_item(member, (was, now))?;
        }
        Ok(moved)
    }

    fn __repr__(&self) -> String {
        format!(
            "PlateSaved(plate={}, replaced={})",
            self.plate.__repr__(),
            match &self.replaced {
                Some(before) => before.__repr__(),
                None => "None".to_string(),
            }
        )
    }
}

/// Record a plate's settings, so a later run in any surface on this machine is told about it
/// by name.
#[pyfunction]
#[pyo3(signature = (name, acquisition, plates_directory = None))]
pub fn save_plate(
    python: Python<'_>,
    name: &str,
    acquisition: &Acquisition,
    plates_directory: Option<PathBuf>,
) -> PyResult<PlateSaved> {
    let (saved, replaced) =
        plate_store::write(name, &acquisition.block(), plates_directory.as_deref())
            .map_err(|refusal| raise_refusal(python, &refusal))?;
    let replaced_members = replaced
        .as_ref()
        .map(|before| plate_store::replacements(&before.members, &saved.members))
        .unwrap_or_default();
    Ok(PlateSaved {
        plate: Plate { inner: saved },
        replaced: replaced.map(|inner| Plate { inner }),
        replaced_members,
    })
}

/// Every plate this machine holds, in name order.
#[pyfunction]
#[pyo3(signature = (plates_directory = None))]
pub fn saved_plates(python: Python<'_>, plates_directory: Option<PathBuf>) -> PyResult<Vec<Plate>> {
    plate_store::saved(plates_directory.as_deref())
        .map(|plates| plates.into_iter().map(|inner| Plate { inner }).collect())
        .map_err(|refusal| raise_refusal(python, &refusal))
}

/// Remove a saved plate. Results already recorded against it carry its members and are
/// unchanged.
#[pyfunction]
#[pyo3(signature = (name, plates_directory = None))]
pub fn forget_plate(
    python: Python<'_>,
    name: &str,
    plates_directory: Option<PathBuf>,
) -> PyResult<String> {
    plate_store::forget(name, plates_directory.as_deref())
        .map(|path| path.display().to_string())
        .map_err(|refusal| raise_refusal(python, &refusal))
}

/// Where saved plates live on this machine.
#[pyfunction]
#[pyo3(signature = (plates_directory = None))]
pub fn plates_folder(python: Python<'_>, plates_directory: Option<PathBuf>) -> PyResult<String> {
    plate_store::directory(plates_directory.as_deref())
        .map(|path| path.display().to_string())
        .map_err(|refusal| raise_refusal(python, &refusal))
}

/// The saved plate a result's block was filled from, as the record carries it.
///
/// Beside the members rather than in place of them. A reader who ignores this entirely still
/// holds every member; what it adds is which plate the answers were typed into and which
/// revision of it ran, so two results taken off one name after somebody edited it differ
/// visibly rather than silently.
#[pyclass(
    frozen,
    skip_from_py_object,
    module = "plateforce",
    name = "PlateProfile"
)]
#[derive(Clone)]
pub struct PlateProfile {
    pub(crate) attribution: plateforce_core::PlateProfileAttribution,
}

#[pymethods]
impl PlateProfile {
    #[getter]
    fn name(&self) -> &str {
        &self.attribution.name
    }

    #[getter]
    fn revision(&self) -> &str {
        &self.attribution.revision
    }

    /// Members the plate states that the caller replaced, as the plate states them. What ran
    /// is in the block, and this is what it displaced, so a reader sees both numbers.
    #[getter]
    fn superseded_members<'py>(&self, python: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let displaced = PyDict::new(python);
        for (member, value) in &self.attribution.superseded_members {
            displaced.set_item(member, value)?;
        }
        Ok(displaced)
    }

    fn __repr__(&self) -> String {
        format!(
            "PlateProfile(name='{}', revision='{}', superseded_members={:?})",
            self.attribution.name, self.attribution.revision, self.attribution.superseded_members
        )
    }
}
