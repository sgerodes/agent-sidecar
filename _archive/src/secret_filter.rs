use std::{collections::BTreeSet, fs, path::Path};

use aho_corasick::AhoCorasick;
use base64::{Engine, engine::general_purpose};
use thiserror::Error;

use crate::config::{PostgresConfig, SecretFilterConfig};

const MIN_SECRET_LEN: usize = 8;
const MIN_FRAGMENT_LEN: usize = 16;
const MAX_SECRET_FILE_BYTES: u64 = 64 * 1024;
const STREAM_TAIL_BYTES: usize = 256;

#[derive(Debug, Clone)]
pub struct SecretFilter {
    matcher: Option<AhoCorasick>,
    patterns: Vec<SecretPattern>,
}

#[derive(Debug, Clone)]
struct SecretPattern {
    label: String,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretDetection {
    pub label: String,
}

#[derive(Debug, Error)]
pub enum SecretFilterError {
    #[error("failed to read protected secret file {path}: {source}")]
    ReadSecretFile {
        path: String,
        source: std::io::Error,
    },

    #[error("failed to build secret matcher: {0}")]
    BuildMatcher(aho_corasick::BuildError),
}

impl SecretFilter {
    pub fn from_config(
        config: &SecretFilterConfig,
        database: Option<&PostgresConfig>,
    ) -> Result<Self, SecretFilterError> {
        let mut candidates = Vec::new();

        if let Some(database) = database {
            candidates.push(("postgres.password".to_owned(), database.password.clone()));
        }

        for (index, secret) in config.canary_secrets.iter().enumerate() {
            candidates.push((format!("canary.{index}"), secret.clone()));
        }

        for path in &config.secret_file_paths {
            candidates.extend(read_secret_file_candidates(path)?);
        }

        Self::new(candidates)
    }

    pub fn new<I>(secrets: I) -> Result<Self, SecretFilterError>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let mut unique = BTreeSet::new();
        let mut patterns = Vec::new();

        for (label, secret) in secrets {
            for variant in secret_variants(&secret) {
                if variant.len() < MIN_SECRET_LEN {
                    continue;
                }

                if unique.insert((label.clone(), variant.clone())) {
                    patterns.push(SecretPattern {
                        label: label.clone(),
                        value: variant,
                    });
                }
            }
        }

        let matcher = if patterns.is_empty() {
            None
        } else {
            Some(
                AhoCorasick::builder()
                    .ascii_case_insensitive(false)
                    .build(patterns.iter().map(|pattern| pattern.value.as_str()))
                    .map_err(SecretFilterError::BuildMatcher)?,
            )
        };

        Ok(Self { matcher, patterns })
    }

    pub fn scan_text(&self, text: &str) -> Option<SecretDetection> {
        self.scan_raw(text)
            .or_else(|| self.scan_raw(&strip_ascii_whitespace(text)))
    }

    fn scan_raw(&self, text: &str) -> Option<SecretDetection> {
        let matcher = self.matcher.as_ref()?;
        let detected = matcher.find(text)?;
        let pattern = &self.patterns[detected.pattern().as_usize()];

        Some(SecretDetection {
            label: pattern.label.clone(),
        })
    }
}

/// Incremental scanner for provider streams where a secret may cross chunk boundaries.
#[derive(Debug, Clone)]
pub struct StreamingSecretScanner {
    filter: SecretFilter,
    tail: String,
}

impl StreamingSecretScanner {
    pub fn new(filter: SecretFilter) -> Self {
        Self {
            filter,
            tail: String::new(),
        }
    }

    pub fn scan_chunk(&mut self, chunk: &str) -> Option<SecretDetection> {
        let combined = format!("{}{}", self.tail, chunk);
        let detection = self.filter.scan_text(&combined);
        self.tail = retain_tail(&combined, STREAM_TAIL_BYTES);
        detection
    }
}

fn read_secret_file_candidates(path: &Path) -> Result<Vec<(String, String)>, SecretFilterError> {
    let metadata = fs::metadata(path).map_err(|source| SecretFilterError::ReadSecretFile {
        path: path.display().to_string(),
        source,
    })?;

    if metadata.len() > MAX_SECRET_FILE_BYTES {
        tracing::warn!(
            path = %path.display(),
            bytes = metadata.len(),
            "skipping oversized secret file for egress filter inventory"
        );
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path).map_err(|source| SecretFilterError::ReadSecretFile {
        path: path.display().to_string(),
        source,
    })?;

    let label_prefix = format!("secret-file:{}", path.display());
    let mut candidates = Vec::new();

    let trimmed = content.trim();
    if (MIN_SECRET_LEN..=512).contains(&trimmed.len()) {
        candidates.push((label_prefix.clone(), trimmed.to_owned()));
    }

    for (index, token) in extract_token_candidates(&content).into_iter().enumerate() {
        candidates.push((format!("{label_prefix}:{index}"), token));
    }

    Ok(candidates)
}

fn extract_token_candidates(content: &str) -> Vec<String> {
    content
        .split(|character: char| {
            !(character.is_ascii_alphanumeric()
                || matches!(character, '-' | '_' | '.' | '/' | '+' | '='))
        })
        .filter(|token| token.len() >= MIN_SECRET_LEN)
        .filter(|token| token.chars().collect::<BTreeSet<_>>().len() >= 6)
        .map(ToOwned::to_owned)
        .collect()
}

fn secret_variants(secret: &str) -> Vec<String> {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut values = BTreeSet::new();
    values.insert(trimmed.to_owned());
    values.insert(strip_ascii_whitespace(trimmed));
    values.insert(general_purpose::STANDARD.encode(trimmed.as_bytes()));
    values.insert(general_purpose::URL_SAFE_NO_PAD.encode(trimmed.as_bytes()));
    values.insert(percent_encode(trimmed));

    if trimmed.len() >= MIN_FRAGMENT_LEN {
        for fragment in sliding_fragments(trimmed, MIN_FRAGMENT_LEN) {
            values.insert(fragment);
        }
    }

    values.into_iter().collect()
}

fn sliding_fragments(value: &str, size: usize) -> Vec<String> {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() < size {
        return Vec::new();
    }

    chars
        .windows(size)
        .step_by(size / 2)
        .map(|window| window.iter().collect())
        .collect()
}

fn strip_ascii_whitespace(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect()
}

fn retain_tail(value: &str, max_chars: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let start = chars.len().saturating_sub(max_chars);
    chars[start..].iter().collect()
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{SecretFilter, StreamingSecretScanner};

    #[test]
    fn detects_exact_secret_without_returning_value() {
        let filter = SecretFilter::new([(
            "db.password".to_owned(),
            "super-secret-token-123".to_owned(),
        )])
        .expect("filter");

        let detection = filter
            .scan_text("the value is super-secret-token-123")
            .expect("detection");

        assert_eq!(detection.label, "db.password");
    }

    #[test]
    fn detects_secret_with_spaces_inserted() {
        let filter = SecretFilter::new([(
            "provider.auth".to_owned(),
            "codex-auth-secret-value".to_owned(),
        )])
        .expect("filter");

        assert!(filter.scan_text("codex-auth- secret-value").is_some());
    }

    #[test]
    fn detects_secret_across_stream_chunks() {
        let filter = SecretFilter::new([(
            "canary".to_owned(),
            "canary-secret-across-boundary".to_owned(),
        )])
        .expect("filter");
        let mut scanner = StreamingSecretScanner::new(filter);

        assert!(scanner.scan_chunk("prefix canary-secret").is_none());
        assert!(scanner.scan_chunk("-across-boundary suffix").is_some());
    }
}
