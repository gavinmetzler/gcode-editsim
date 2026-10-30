//! Checks the workspace's dependency rules.
//!
//! The rules (`rules.toml`, documented in ARCHITECTURE.md section 5) say
//! which workspace crates each crate may depend on. [`check`] compares
//! them with the dependency data from `cargo metadata`, which already
//! resolves renamed, platform-specific and workspace-inherited
//! dependencies, so none of those can bypass the rules.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The rules file shipped with this crate.
pub const RULES: &str = include_str!("../rules.toml");

/// The parts of `cargo metadata --format-version 1 --no-deps` output used
/// here.
#[derive(Debug, Deserialize)]
pub struct Metadata {
    /// Workspace members (with `--no-deps`, exactly the members).
    pub packages: Vec<Package>,
    /// Directory containing the workspace's root `Cargo.toml`.
    pub workspace_root: PathBuf,
}

/// One workspace member.
#[derive(Debug, Deserialize)]
pub struct Package {
    /// Package name.
    pub name: String,
    /// Path to its `Cargo.toml`.
    pub manifest_path: PathBuf,
    /// Its declared dependencies.
    pub dependencies: Vec<Dependency>,
}

/// One declared dependency.
#[derive(Debug, Deserialize)]
pub struct Dependency {
    /// The depended-on package's real name (not a rename).
    pub name: String,
    /// `None` for normal, `Some("dev")` or `Some("build")`.
    pub kind: Option<String>,
    /// Platform condition, e.g. `cfg(windows)`, if any.
    pub target: Option<String>,
    /// Directory of a path dependency.
    pub path: Option<PathBuf>,
}

/// The rules file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// Allowed normal/build workspace dependencies per crate.
    pub allowed: BTreeMap<String, Vec<String>>,
    /// Rules for dev dependencies.
    pub dev: DevRules,
}

/// Rules for test-only dependencies.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevRules {
    /// Top-level directories whose crates dev dependencies may use.
    pub allowed_dirs: Vec<String>,
}

impl Rules {
    /// Parses a rules file.
    pub fn parse(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }
}

/// One broken rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    /// A workspace crate the rules do not mention.
    Unclassified {
        /// The crate.
        krate: String,
    },
    /// The rules mention a crate that is not in the workspace.
    UnknownInRules {
        /// The name in the rules.
        name: String,
    },
    /// A normal or build dependency the rules do not allow.
    Forbidden {
        /// The depending crate.
        krate: String,
        /// The workspace crate it depends on.
        dependency: String,
        /// `normal` or `build`, plus any platform condition.
        kind: String,
        /// What the rules allow for this crate.
        allowed: Vec<String>,
    },
    /// A dev dependency on a crate outside the allowed directories.
    ForbiddenDev {
        /// The depending crate.
        krate: String,
        /// The workspace crate it depends on.
        dependency: String,
        /// Where that crate lives, relative to the workspace root.
        location: String,
    },
    /// A path dependency on something that is not a workspace member (e.g.
    /// `app/` before it joins the workspace).
    OutsideWorkspace {
        /// The depending crate.
        krate: String,
        /// The dependency's name.
        dependency: String,
        /// The dependency's path.
        path: String,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Violation::Unclassified { krate } => write!(
                f,
                "{krate} is not listed in tools/depcheck/rules.toml; add it with the crates it may \
                 depend on (see ARCHITECTURE.md section 5)"
            ),
            Violation::UnknownInRules { name } => write!(
                f,
                "tools/depcheck/rules.toml mentions {name}, which is not a workspace crate"
            ),
            Violation::Forbidden {
                krate,
                dependency,
                kind,
                allowed,
            } => {
                let allowed = if allowed.is_empty() {
                    "no workspace crates".to_owned()
                } else {
                    allowed.join(", ")
                };
                write!(
                    f,
                    "{krate} has a {kind} dependency on {dependency}, which ARCHITECTURE.md \
                     section 5 does not allow ({krate} may only use: {allowed})"
                )
            }
            Violation::ForbiddenDev {
                krate,
                dependency,
                location,
            } => write!(
                f,
                "{krate} has a dev dependency on {dependency} ({location}); test-only \
                 dependencies may only use crates in the directories allowed by rules.toml"
            ),
            Violation::OutsideWorkspace {
                krate,
                dependency,
                path,
            } => write!(
                f,
                "{krate} depends on {dependency} at {path}, which is not a workspace member"
            ),
        }
    }
}

fn kind_name(dep: &Dependency) -> String {
    let kind = dep.kind.as_deref().unwrap_or("normal");
    match &dep.target {
        Some(target) => format!("{kind} ({target})"),
        None => kind.to_owned(),
    }
}

/// The first path component of `path` relative to `root`, e.g. `core`.
fn top_dir(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .and_then(|p| p.components().next())
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Every rule broken by the workspace described by `metadata`.
pub fn check(metadata: &Metadata, rules: &Rules) -> Vec<Violation> {
    let mut violations = Vec::new();
    let members: BTreeMap<&str, &Package> = metadata
        .packages
        .iter()
        .map(|p| (p.name.as_str(), p))
        .collect();
    let member_dirs: BTreeSet<&Path> = metadata
        .packages
        .iter()
        .filter_map(|p| p.manifest_path.parent())
        .collect();

    for name in rules.allowed.keys().chain(rules.allowed.values().flatten()) {
        if !members.contains_key(name.as_str()) {
            violations.push(Violation::UnknownInRules { name: name.clone() });
        }
    }

    for package in &metadata.packages {
        let Some(allowed) = rules.allowed.get(&package.name) else {
            violations.push(Violation::Unclassified {
                krate: package.name.clone(),
            });
            continue;
        };
        for dep in &package.dependencies {
            // A workspace crate is always a path dependency; a registry crate
            // that happens to share a member's name is not one of ours.
            let member = dep
                .path
                .as_ref()
                .and_then(|_| members.get(dep.name.as_str()));
            match member {
                Some(target) => {
                    if dep.kind.as_deref() == Some("dev") {
                        let location = top_dir(
                            target.manifest_path.parent().unwrap_or(Path::new("")),
                            &metadata.workspace_root,
                        );
                        if !rules.dev.allowed_dirs.contains(&location) {
                            violations.push(Violation::ForbiddenDev {
                                krate: package.name.clone(),
                                dependency: dep.name.clone(),
                                location,
                            });
                        }
                    } else if !allowed.contains(&dep.name) {
                        violations.push(Violation::Forbidden {
                            krate: package.name.clone(),
                            dependency: dep.name.clone(),
                            kind: kind_name(dep),
                            allowed: allowed.clone(),
                        });
                    }
                }
                None => {
                    // A path dependency on a crate in the repository that is
                    // not a member (or a member under another name).
                    if let Some(path) = &dep.path {
                        if !member_dirs.contains(path.as_path()) {
                            violations.push(Violation::OutsideWorkspace {
                                krate: package.name.clone(),
                                dependency: dep.name.clone(),
                                path: path.display().to_string(),
                            });
                        }
                    }
                }
            }
        }
    }
    violations
}

#[cfg(test)]
mod tests;
