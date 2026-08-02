//! What a run reads, and what it calls each trial it read.
//!
//! Two naming conventions already exist in this repository, so a loop that guessed a third
//! would put a silent default under every count in the product. The format and the identity
//! are both declared once for the run and neither is inferred from a file.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use plateforce_core::{read_delimited_column, ColumnReadReport, ReadError, Trial};
use serde::{Deserialize, Serialize};

/// How every file in one run is read.
///
/// One declaration for the whole run rather than one per file: a 244-file run that inferred
/// the sample rate per file could produce 244 different rates from one recording session
/// without saying so.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SourceFormat {
    pub delimiter: char,
    pub force_column_index: usize,
    pub sample_rate_hz: f64,
    /// Which names in the directory are trials, matched against the end of the file name so
    /// a compound suffix like `force.txt` is expressible. No default, because a walk that
    /// filtered silently would drop files out of the denominator with nothing recording it,
    /// and a walk that did not filter would refuse a README and call that a data failure.
    pub trial_file_suffixes: Vec<String>,
}

impl SourceFormat {
    /// The suffix this name ends with, longest first so `force.txt` wins over `txt`.
    fn matching_suffix<'a>(&'a self, file_name: &str) -> Option<&'a str> {
        let mut ordered: Vec<&String> = self.trial_file_suffixes.iter().collect();
        ordered.sort_by_key(|suffix| std::cmp::Reverse(suffix.len()));
        ordered
            .into_iter()
            .find(|suffix| file_name.len() > suffix.len() && file_name.ends_with(suffix.as_str()))
            .map(String::as_str)
    }
}

/// Where the identity of a trial comes from.
///
/// `FileStem` gives a run exactly one unit of analysis, the trial. A declared pattern gives
/// it a subject as well, and every statistic whose unit of analysis is the subject needs one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TrialIdentity {
    FileStem,
    DeclaredPattern { template: String },
}

impl TrialIdentity {
    /// How the run says it named its trials, carried into the `run` row so the choice travels.
    pub fn describe(&self) -> String {
        match self {
            TrialIdentity::FileStem => "file_stem".to_string(),
            TrialIdentity::DeclaredPattern { template } => {
                format!("declared_pattern {template}")
            }
        }
    }

    pub fn is_declared_pattern(&self) -> bool {
        matches!(self, TrialIdentity::DeclaredPattern { .. })
    }
}

/// One subject, and the occasion their trials were recorded on where a pattern named one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct SubjectKey {
    pub subject: String,
    pub occasion: Option<String>,
}

impl SubjectKey {
    pub fn label(&self) -> String {
        match &self.occasion {
            Some(occasion) => format!("{}/{}", self.subject, occasion),
            None => self.subject.clone(),
        }
    }
}

/// One trial's bytes, from a directory or from a browser that has no filesystem.
#[derive(Debug, Clone, PartialEq)]
pub enum TrialSource {
    Path(PathBuf),
    Memory { name: String, text: String },
}

impl TrialSource {
    pub fn file_name(&self) -> String {
        match self {
            TrialSource::Path(path) => path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string(),
            TrialSource::Memory { name, .. } => name.clone(),
        }
    }

    /// The path as walked. Empty for an in-memory source, which has none.
    pub fn display_path(&self) -> String {
        match self {
            TrialSource::Path(path) => path.display().to_string(),
            TrialSource::Memory { .. } => String::new(),
        }
    }

    pub fn read(&self, format: &SourceFormat) -> Result<(Trial, ColumnReadReport), ReadError> {
        let text = match self {
            TrialSource::Path(path) => {
                std::fs::read_to_string(path).map_err(|source| ReadError::Io {
                    path: path.display().to_string(),
                    source,
                })?
            }
            TrialSource::Memory { text, .. } => text.clone(),
        };
        let (values, report) =
            read_delimited_column(&text, format.delimiter, format.force_column_index)?;
        Ok((Trial::new(values, format.sample_rate_hz)?, report))
    }
}

/// One trial the run walked, with whatever the identity resolved about it.
#[derive(Debug, Clone)]
pub struct TrialEntry {
    pub source: TrialSource,
    pub subject: Option<SubjectKey>,
    pub trial_label: Option<String>,
}

/// A file the run counted and could not name, kept so it can be refused rather than skipped.
#[derive(Debug, Clone, PartialEq)]
pub struct UnidentifiedFile {
    pub file_name: String,
    pub source_path: String,
    /// What would resolve it: the template that did not match, or the ids it collided with.
    pub reason: UnidentifiedReason,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnidentifiedReason {
    PatternDidNotMatch {
        template: String,
    },
    /// Two files resolved to one id, so the table cannot be keyed. Neither is preferred.
    DuplicateTrialId {
        trial_id: String,
        other: String,
    },
}

impl UnidentifiedFile {
    pub fn message(&self) -> String {
        match &self.reason {
            UnidentifiedReason::PatternDidNotMatch { template } => format!(
                "{} does not match the declared pattern {}, so this run has no name for it",
                self.file_name, template
            ),
            UnidentifiedReason::DuplicateTrialId { trial_id, other } => format!(
                "{} and {} both resolve to the trial id {}, so neither can key a row",
                self.file_name, other, trial_id
            ),
        }
    }

    pub fn parameter(&self) -> String {
        match &self.reason {
            UnidentifiedReason::PatternDidNotMatch { template } => template.clone(),
            UnidentifiedReason::DuplicateTrialId { trial_id, .. } => trial_id.clone(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WalkError {
    #[error("reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("a run declares which file names are trials, and this one declares none")]
    NoTrialFileSuffixes,
}

/// Every trial a run walked, each with an identity. May span subjects and occasions.
#[derive(Debug, Clone)]
pub struct TrialSet {
    entries: BTreeMap<String, TrialEntry>,
    pub identity: TrialIdentity,
    pub format: SourceFormat,
    /// Files carrying a declared trial suffix. The denominator every count is taken over.
    pub files_found: usize,
    pub unidentified: Vec<UnidentifiedFile>,
}

impl TrialSet {
    /// Walk a directory. Sorted by path, so a run is reproducible across filesystems that
    /// hand back directory entries in different orders.
    pub fn walk(
        root: &Path,
        format: &SourceFormat,
        identity: &TrialIdentity,
    ) -> Result<Self, WalkError> {
        if format.trial_file_suffixes.is_empty() {
            return Err(WalkError::NoTrialFileSuffixes);
        }
        let mut candidates: Vec<PathBuf> = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            let listing = std::fs::read_dir(&directory).map_err(|source| WalkError::Io {
                path: directory.display().to_string(),
                source,
            })?;
            for entry in listing {
                let entry = entry.map_err(|source| WalkError::Io {
                    path: directory.display().to_string(),
                    source,
                })?;
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                    continue;
                }
                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                if format.matching_suffix(name).is_some() {
                    candidates.push(path);
                }
            }
        }
        candidates.sort();
        Ok(Self::assemble(
            candidates.into_iter().map(TrialSource::Path).collect(),
            format,
            identity,
        ))
    }

    /// The same set from named text, for a surface with no filesystem. One engine serves both.
    pub fn from_sources(
        sources: Vec<(String, String)>,
        format: &SourceFormat,
        identity: &TrialIdentity,
    ) -> Result<Self, WalkError> {
        if format.trial_file_suffixes.is_empty() {
            return Err(WalkError::NoTrialFileSuffixes);
        }
        let mut candidates: Vec<TrialSource> = sources
            .into_iter()
            .filter(|(name, _)| format.matching_suffix(name).is_some())
            .map(|(name, text)| TrialSource::Memory { name, text })
            .collect();
        candidates.sort_by_key(TrialSource::file_name);
        Ok(Self::assemble(candidates, format, identity))
    }

    fn assemble(
        candidates: Vec<TrialSource>,
        format: &SourceFormat,
        identity: &TrialIdentity,
    ) -> Self {
        let files_found = candidates.len();
        let mut entries: BTreeMap<String, TrialEntry> = BTreeMap::new();
        let mut claimed_by: BTreeMap<String, String> = BTreeMap::new();
        let mut unidentified: Vec<UnidentifiedFile> = Vec::new();
        let mut collided: BTreeSet<String> = BTreeSet::new();

        for source in candidates {
            let file_name = source.file_name();
            let suffix = format.matching_suffix(&file_name).unwrap_or_default();
            let stem = file_name[..file_name.len() - suffix.len()]
                .trim_end_matches('.')
                .to_string();

            let parsed = match identity {
                TrialIdentity::FileStem => Some((stem.clone(), None, None)),
                TrialIdentity::DeclaredPattern { template } => {
                    parse_template(template, &stem).map(|fields| {
                        let subject = fields.get("subject").map(|subject| SubjectKey {
                            subject: subject.clone(),
                            occasion: fields.get("occasion").cloned(),
                        });
                        (stem.clone(), subject, fields.get("trial").cloned())
                    })
                }
            };

            let Some((trial_id, subject, trial_label)) = parsed else {
                let TrialIdentity::DeclaredPattern { template } = identity else {
                    unreachable!("only a declared pattern can fail to parse");
                };
                unidentified.push(UnidentifiedFile {
                    file_name,
                    source_path: source.display_path(),
                    reason: UnidentifiedReason::PatternDidNotMatch {
                        template: template.clone(),
                    },
                });
                continue;
            };

            // Two files resolving to one id would key one row and lose the other, so both
            // are named and neither is preferred over the other.
            if let Some(first) = claimed_by.get(&trial_id) {
                unidentified.push(UnidentifiedFile {
                    file_name: file_name.clone(),
                    source_path: source.display_path(),
                    reason: UnidentifiedReason::DuplicateTrialId {
                        trial_id: trial_id.clone(),
                        other: first.clone(),
                    },
                });
                collided.insert(trial_id.clone());
                continue;
            }
            claimed_by.insert(trial_id.clone(), file_name);
            entries.insert(
                trial_id,
                TrialEntry {
                    source,
                    subject,
                    trial_label,
                },
            );
        }

        for trial_id in collided {
            if let Some(entry) = entries.remove(&trial_id) {
                let other = unidentified
                    .iter()
                    .find(|file| file.parameter() == trial_id)
                    .map(|file| file.file_name.clone())
                    .unwrap_or_default();
                unidentified.push(UnidentifiedFile {
                    file_name: entry.source.file_name(),
                    source_path: entry.source.display_path(),
                    reason: UnidentifiedReason::DuplicateTrialId { trial_id, other },
                });
            }
        }
        unidentified.sort_by(|left, right| left.file_name.cmp(&right.file_name));

        Self {
            entries,
            identity: identity.clone(),
            format: format.clone(),
            files_found,
            unidentified,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &TrialEntry)> {
        self.entries.iter()
    }

    pub fn get(&self, trial_id: &str) -> Option<&TrialEntry> {
        self.entries.get(trial_id)
    }

    pub fn trial_ids(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    /// Whether a path may be written into `results`. A declared pattern already carries the
    /// subject, and the source corpus keeps athlete names in its directory names, so the
    /// column is omitted as an act and the `run` row records that it was.
    pub fn writes_source_path(&self) -> bool {
        !self.identity.is_declared_pattern()
    }
}

/// One subject's trials from one occasion. The grouping every reliability figure is taken
/// over, and the meaning the registry already gives the word.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub key: SubjectKey,
    pub trial_ids: Vec<String>,
}

impl Session {
    /// `None` when the run declared no pattern, because a set with no declared grouping has
    /// no subject to group by and treating each trial as a subject would invent one.
    pub fn group(set: &TrialSet) -> Option<Vec<Session>> {
        if !set.identity.is_declared_pattern() {
            return None;
        }
        let mut grouped: BTreeMap<SubjectKey, Vec<String>> = BTreeMap::new();
        for (trial_id, entry) in set.iter() {
            if let Some(key) = &entry.subject {
                grouped
                    .entry(key.clone())
                    .or_default()
                    .push(trial_id.clone());
            }
        }
        Some(
            grouped
                .into_iter()
                .map(|(key, trial_ids)| Session { key, trial_ids })
                .collect(),
        )
    }
}

/// Pull the named fields out of a name under a template such as `AT{subject}_{trial}`.
///
/// Returns `None` when the literal text between the placeholders is not where the template
/// says it is, which is the case Task-level callers refuse by name rather than skip.
fn parse_template(template: &str, name: &str) -> Option<BTreeMap<String, String>> {
    let tokens = tokenise(template)?;
    let mut fields = BTreeMap::new();
    let mut rest = name;
    let mut index = 0usize;

    while index < tokens.len() {
        match &tokens[index] {
            Token::Literal(text) => {
                rest = rest.strip_prefix(text.as_str())?;
                index += 1;
            }
            Token::Field(field) => {
                let following = tokens.get(index + 1);
                let captured = match following {
                    Some(Token::Literal(text)) => {
                        let cut = rest.find(text.as_str())?;
                        let (captured, remainder) = rest.split_at(cut);
                        rest = remainder;
                        captured
                    }
                    // A field is the last thing in the template, so it takes what is left.
                    None => {
                        let captured = rest;
                        rest = "";
                        captured
                    }
                    Some(Token::Field(_)) => return None,
                };
                if captured.is_empty() {
                    return None;
                }
                fields.insert(field.clone(), captured.to_string());
                index += 1;
            }
        }
    }
    if rest.is_empty() {
        Some(fields)
    } else {
        None
    }
}

enum Token {
    Literal(String),
    Field(String),
}

/// Two placeholders with no literal between them cannot be split, so the template is refused
/// rather than resolved by a rule nobody declared.
fn tokenise(template: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut literal = String::new();
    let mut characters = template.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '{' => {
                if !literal.is_empty() {
                    tokens.push(Token::Literal(std::mem::take(&mut literal)));
                }
                let mut field = String::new();
                for inner in characters.by_ref() {
                    if inner == '}' {
                        break;
                    }
                    field.push(inner);
                }
                if field.is_empty() {
                    return None;
                }
                if matches!(tokens.last(), Some(Token::Field(_))) {
                    return None;
                }
                tokens.push(Token::Field(field));
            }
            '}' => return None,
            other => literal.push(other),
        }
    }
    if !literal.is_empty() {
        tokens.push(Token::Literal(literal));
    }
    if tokens.is_empty() {
        None
    } else {
        Some(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_pulls_the_subject_and_the_trial_out_of_a_name() {
        let fields = parse_template("AT{subject}_{trial}", "AT01_6").unwrap();
        assert_eq!(fields.get("subject").map(String::as_str), Some("01"));
        assert_eq!(fields.get("trial").map(String::as_str), Some("6"));
    }

    #[test]
    fn a_name_the_template_does_not_describe_yields_nothing() {
        assert!(parse_template("AT{subject}_{trial}", "notes").is_none());
        assert!(parse_template("AT{subject}_{trial}", "AT016").is_none());
    }

    #[test]
    fn two_placeholders_with_nothing_between_them_are_refused() {
        assert!(tokenise("{subject}{trial}").is_none());
        assert!(tokenise("AT{}_{trial}").is_none());
    }

    #[test]
    fn the_longest_declared_suffix_names_the_stem() {
        let format = SourceFormat {
            delimiter: '\t',
            force_column_index: 0,
            sample_rate_hz: 1200.0,
            trial_file_suffixes: vec!["txt".to_string(), "force.txt".to_string()],
        };
        assert_eq!(
            format.matching_suffix("subject01_trial1.force.txt"),
            Some("force.txt")
        );
    }
}
