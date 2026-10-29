//! Data-driven code tables loaded from TOML.

use std::collections::BTreeMap;

use serde::Deserialize;
use thiserror::Error;

use crate::code::CodeKey;

/// Everything a dialect declares about one G or M code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeDef {
    /// The code itself.
    pub code: CodeKey,
    /// Modal group, if the code belongs to one.
    pub group: Option<u16>,
    /// `true` if the code stays in effect until another code of its group
    /// replaces it; `false` for one-shot codes.
    pub modal: bool,
    /// Name of the dialect's routine that carries the code out.
    pub handler: String,
    /// Address letters the code needs in the same block.
    pub required_words: Vec<char>,
    /// Address letters the code may use in the same block.
    pub optional_words: Vec<char>,
    /// Human-readable description.
    pub description: Option<String>,
}

/// A dialect's table of supported codes.
///
/// Loaded from TOML text shaped like this:
///
/// ```toml
/// [dialect]
/// id = "example"
/// variant = "basic"          # optional
///
/// [[g]]
/// code = "G01"
/// group = 1
/// modal = true
/// handler = "linear"
/// description = "Linear move"            # optional
/// words = { optional = ["F"] }           # optional
///
/// [[m]]
/// code = "M30"
/// handler = "program_end"
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeTable {
    id: String,
    variant: Option<String>,
    codes: BTreeMap<CodeKey, CodeDef>,
}

/// Why a code table could not be loaded.
#[derive(Debug, Error)]
pub enum CodeTableError {
    /// The text is not valid TOML or does not have the expected shape.
    #[error("invalid code table: {0}")]
    Toml(#[from] toml::de::Error),
    /// A `code` value that is not a letter followed by a number.
    #[error("invalid code {0:?}")]
    InvalidCode(String),
    /// A code listed in the wrong section, e.g. `M3` under `[[g]]`.
    #[error("{code} is listed under [[{section}]]")]
    WrongSection {
        /// The code.
        code: String,
        /// The section it was found in.
        section: char,
    },
    /// The same code defined twice.
    #[error("{0} is defined more than once")]
    Duplicate(String),
    /// A modal code without a group.
    #[error("{0} is modal but has no group")]
    ModalWithoutGroup(String),
    /// A word letter that is not a single letter A-Z.
    #[error("{code}: {word:?} is not a single letter")]
    InvalidWord {
        /// The code.
        code: String,
        /// The offending entry.
        word: String,
    },
    /// An empty handler name.
    #[error("{0} has no handler")]
    MissingHandler(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTable {
    dialect: RawInfo,
    #[serde(default)]
    g: Vec<RawCode>,
    #[serde(default)]
    m: Vec<RawCode>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInfo {
    id: String,
    #[serde(default)]
    variant: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCode {
    code: String,
    #[serde(default)]
    group: Option<u16>,
    #[serde(default)]
    modal: bool,
    handler: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    words: RawWords,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawWords {
    #[serde(default)]
    required: Vec<String>,
    #[serde(default)]
    optional: Vec<String>,
}

impl CodeTable {
    /// Parses and checks a code table.
    pub fn from_toml_str(text: &str) -> Result<Self, CodeTableError> {
        let raw: RawTable = toml::from_str(text)?;
        let mut codes = BTreeMap::new();
        let sections = raw
            .g
            .into_iter()
            .map(|c| ('G', c))
            .chain(raw.m.into_iter().map(|c| ('M', c)));
        for (section, entry) in sections {
            let def = check_entry(section, entry)?;
            if codes.contains_key(&def.code) {
                return Err(CodeTableError::Duplicate(def.code.to_string()));
            }
            codes.insert(def.code, def);
        }
        Ok(Self {
            id: raw.dialect.id,
            variant: raw.dialect.variant,
            codes,
        })
    }

    /// The dialect id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The dialect variant, if given.
    pub fn variant(&self) -> Option<&str> {
        self.variant.as_deref()
    }

    /// Looks up a code.
    pub fn get(&self, code: CodeKey) -> Option<&CodeDef> {
        self.codes.get(&code)
    }

    /// All codes, ordered by letter then number.
    pub fn iter(&self) -> impl Iterator<Item = &CodeDef> {
        self.codes.values()
    }

    /// Number of codes in the table.
    pub fn len(&self) -> usize {
        self.codes.len()
    }

    /// `true` if the table defines no codes.
    pub fn is_empty(&self) -> bool {
        self.codes.is_empty()
    }
}

fn check_entry(section: char, entry: RawCode) -> Result<CodeDef, CodeTableError> {
    let code = CodeKey::parse(&entry.code)
        .ok_or_else(|| CodeTableError::InvalidCode(entry.code.clone()))?;
    if code.letter() != section {
        return Err(CodeTableError::WrongSection {
            code: code.to_string(),
            section: section.to_ascii_lowercase(),
        });
    }
    if entry.modal && entry.group.is_none() {
        return Err(CodeTableError::ModalWithoutGroup(code.to_string()));
    }
    if entry.handler.trim().is_empty() {
        return Err(CodeTableError::MissingHandler(code.to_string()));
    }
    let letters = |words: Vec<String>| -> Result<Vec<char>, CodeTableError> {
        words
            .into_iter()
            .map(|w| {
                let mut chars = w.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) if c.is_ascii_alphabetic() => Ok(c.to_ascii_uppercase()),
                    _ => Err(CodeTableError::InvalidWord {
                        code: code.to_string(),
                        word: w,
                    }),
                }
            })
            .collect()
    };
    Ok(CodeDef {
        code,
        group: entry.group,
        modal: entry.modal,
        handler: entry.handler,
        required_words: letters(entry.words.required)?,
        optional_words: letters(entry.words.optional)?,
        description: entry.description,
    })
}
