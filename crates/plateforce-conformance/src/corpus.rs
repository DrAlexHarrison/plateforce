//! Locating trials on disk.
//!
//! Directory names in the source corpus are athlete names. Nothing in this module
//! returns or records a path component: a trial is addressed by subject number and
//! trial number, taken from the file name, and the directory it sat in is discarded on
//! the way past.

use plateforce_core::{read_trial_from_path, Trial};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// How a corpus export is laid out.
#[derive(Debug, Clone, Copy)]
pub struct CorpusFormat {
    pub file_prefix: &'static str,
    pub delimiter: char,
    pub force_column_index: usize,
    pub sample_rate_hz: f64,
}

impl Default for CorpusFormat {
    fn default() -> Self {
        Self {
            file_prefix: "AT",
            delimiter: '\t',
            force_column_index: 2,
            sample_rate_hz: 1200.0,
        }
    }
}

/// Subject and trial number parsed out of a file name of the form `AT<subject>_<trial>`.
pub fn identify(file_name: &str, prefix: &str) -> Option<(u32, u32)> {
    let stem = file_name.strip_suffix(".txt")?.strip_prefix(prefix)?;
    let (subject, trial) = stem.split_once('_')?;
    Some((subject.parse().ok()?, trial.parse().ok()?))
}

/// Index every trial under a directory, keyed by subject and trial number.
pub fn index_corpus(
    root: &Path,
    format: &CorpusFormat,
) -> std::io::Result<BTreeMap<(u32, u32), PathBuf>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if let Some(key) = identify(name, format.file_prefix) {
                found.insert(key, path);
            }
        }
    }
    Ok(found)
}

/// A corpus already indexed, ready to hand trials to the comparison.
pub struct Corpus {
    paths: BTreeMap<(u32, u32), PathBuf>,
    format: CorpusFormat,
}

impl Corpus {
    pub fn open(root: &Path, format: CorpusFormat) -> std::io::Result<Self> {
        Ok(Self {
            paths: index_corpus(root, &format)?,
            format,
        })
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn trial(&self, subject: u32, trial: u32) -> Option<Trial> {
        let path = self.paths.get(&(subject, trial))?;
        read_trial_from_path(
            path,
            self.format.delimiter,
            self.format.force_column_index,
            self.format.sample_rate_hz,
        )
        .ok()
        .map(|(trace, _)| trace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trial_is_identified_by_number_and_never_by_the_directory_it_sat_in() {
        assert_eq!(identify("AT01_6.txt", "AT"), Some((1, 6)));
        assert_eq!(identify("AT13_3.txt", "AT"), Some((13, 3)));
        assert_eq!(identify("notes.txt", "AT"), None);
        assert_eq!(identify("AT01_6.csv", "AT"), None);
    }
}
