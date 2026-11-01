//! Reading and saving programs, and finding machine files.
//!
//! Paths come from the user (a file dialog, or the app's own folders).

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::BackendError;

/// The largest program the app reads, in bytes (256 MiB). Larger files are
/// refused rather than risk running out of memory.
pub const MAX_PROGRAM_BYTES: u64 = 256 * 1024 * 1024;

/// The largest machine file the app reads, in bytes (1 MiB).
pub const MAX_MACHINE_FILE_BYTES: u64 = 1024 * 1024;

fn io_error(path: &Path, source: std::io::Error) -> BackendError {
    BackendError::Io {
        path: path.display().to_string(),
        source,
    }
}

/// Reads a whole file, refusing files larger than `limit` bytes.
pub fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, BackendError> {
    let file = fs::File::open(path).map_err(|e| io_error(path, e))?;
    let mut bytes = Vec::new();
    // Read one byte past the limit to tell "exactly at" from "over".
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| io_error(path, e))?;
    if bytes.len() as u64 > limit {
        return Err(BackendError::FileTooLarge {
            path: path.display().to_string(),
            limit,
        });
    }
    Ok(bytes)
}

/// Reads a program file.
pub fn read_program(path: &Path) -> Result<Vec<u8>, BackendError> {
    read_limited(path, MAX_PROGRAM_BYTES)
}

/// Saves a program file. The text is written to a temporary file next to
/// it, which then replaces the original, so a failed save never leaves a
/// half-written program behind.
///
/// - A symlink is written through: the file it points at is replaced and
///   the link stays a link.
/// - The file keeps its permissions (a private file stays private), where
///   the file system allows it. A new file gets the usual permissions for
///   new files.
/// - A leftover temporary file from an earlier failed save is removed, and
///   never followed if it is a link.
pub fn save_program(path: &Path, contents: &[u8]) -> Result<(), BackendError> {
    // The real file behind any symlinks; a file that does not exist yet is
    // saved where asked.
    let target = match fs::canonicalize(path) {
        Ok(real) => real,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => path.to_path_buf(),
        Err(e) => return Err(io_error(path, e)),
    };
    let file_name = target
        .file_name()
        .ok_or_else(|| io_error(path, std::io::ErrorKind::InvalidInput.into()))?;
    let mut temp_name = std::ffi::OsString::from(".");
    temp_name.push(file_name);
    temp_name.push(".saving");
    let temp = target.with_file_name(temp_name);
    let permissions = fs::metadata(&target).ok().map(|m| m.permissions());

    let write = || -> std::io::Result<()> {
        match fs::remove_file(&temp) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let mut options = fs::OpenOptions::new();
        // `create_new` refuses to open anything already there, so a link
        // planted after the removal above is not followed either.
        options.write(true).create_new(true);
        #[cfg(unix)]
        if permissions.is_some() {
            // Owner-only while writing; the file's own permissions are set
            // below, before it replaces the original.
            std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        }
        let mut f = options.open(&temp)?;
        f.write_all(contents)?;
        if let Some(p) = &permissions {
            // Best effort: some file systems (the Flatpak document portal)
            // may refuse; the save itself must not fail for that.
            let _ = f.set_permissions(p.clone());
        }
        f.sync_all()?;
        fs::rename(&temp, &target)
    };
    write().map_err(|e| {
        let _ = fs::remove_file(&temp);
        io_error(path, e)
    })
}

/// Where a machine file comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineSource {
    /// An example shipped with the app, by id.
    Shipped(&'static str),
    /// A file in the user's machine folder.
    User(PathBuf),
}

/// A machine file the user can choose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFile {
    /// Name to show: the file's `[machine] name`, or the file name if it
    /// cannot be read.
    pub label: String,
    /// Where it is.
    pub source: MachineSource,
    /// Why it cannot be used, if it cannot.
    pub problem: Option<String>,
}

/// The examples shipped with the app: `(id, text)`.
pub const SHIPPED_MACHINES: &[(&str, &str)] = &[
    ("generic_3axis_mill", machine_mill::EXAMPLE_DEFINITION),
    (
        "generic_3axis_router",
        machine_mill::ROUTER_EXAMPLE_DEFINITION,
    ),
    ("generic_2axis_lathe", machine_lathe::EXAMPLE_DEFINITION),
];

/// The shipped examples copied into a new machine folder, by id: the
/// machine types the app simulates (the lathe is a stub until M8). Their
/// file names (`<id>.toml`) sort with the mill first, so it is the default.
pub const SEEDED_MACHINES: &[&str] = &["generic_3axis_mill", "generic_3axis_router"];

/// The text of a shipped example, by id.
pub fn shipped_machine(id: &str) -> Option<&'static str> {
    SHIPPED_MACHINES
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, text)| *text)
}

fn describe(text: &str) -> (Option<String>, Option<String>) {
    match crate::load_machine(text) {
        Ok(m) => (Some(m.name().to_owned()), None),
        Err(e) => (None, Some(e.to_string())),
    }
}

/// The `*.toml` files in `dir` (not its subfolders), sorted by path. A
/// missing `dir` has none.
fn machine_paths(dir: &Path) -> Result<Vec<PathBuf>, BackendError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_error(dir, e)),
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("toml"))
        })
        .collect();
    paths.sort();
    Ok(paths)
}

/// Gives a machine folder its starting files: creates `dir` if missing and,
/// if it has no `*.toml` files, copies the [`SEEDED_MACHINES`] examples in
/// as `<id>.toml`. Never overwrites a file. Returns whether it copied
/// anything. The examples are built into the app, so nothing is fetched.
pub fn seed_machine_folder(dir: &Path) -> Result<bool, BackendError> {
    fs::create_dir_all(dir).map_err(|e| io_error(dir, e))?;
    if !machine_paths(dir)?.is_empty() {
        return Ok(false);
    }
    let mut first_error = None;
    for id in SEEDED_MACHINES {
        let Some(text) = shipped_machine(id) else {
            continue;
        };
        let path = dir.join(format!("{id}.toml"));
        let write = || -> std::io::Result<()> {
            // `create_new` refuses to replace anything already there.
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            f.write_all(text.as_bytes())
        };
        if let Err(e) = write() {
            first_error.get_or_insert(io_error(&path, e));
        }
    }
    match first_error {
        Some(e) => Err(e),
        None => Ok(true),
    }
}

/// The machine files the user can choose: every `*.toml` file in
/// `user_dir` (not its subfolders), sorted by file name. If there are none
/// (or no folder), the shipped examples instead, so there is always
/// something to choose. Files that cannot be used are listed with the
/// problem, so the user can see why.
pub fn list_machine_files(user_dir: &Path) -> Result<Vec<MachineFile>, BackendError> {
    let paths = machine_paths(user_dir)?;
    if paths.is_empty() {
        return Ok(SHIPPED_MACHINES
            .iter()
            .map(|(id, text)| {
                let (name, problem) = describe(text);
                MachineFile {
                    label: name.unwrap_or_else(|| (*id).to_owned()),
                    source: MachineSource::Shipped(id),
                    problem,
                }
            })
            .collect());
    }
    let mut list = Vec::with_capacity(paths.len());
    for path in paths {
        let file_label = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let (name, problem) = match read_limited(&path, MAX_MACHINE_FILE_BYTES) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => describe(&text),
                Err(_) => (None, Some("not UTF-8 text".to_owned())),
            },
            Err(e) => (None, Some(e.to_string())),
        };
        list.push(MachineFile {
            label: name.unwrap_or(file_label),
            source: MachineSource::User(path),
            problem,
        });
    }
    Ok(list)
}

/// The text of a machine file the user chose from [`list_machine_files`].
pub fn read_machine_file(source: &MachineSource) -> Result<String, BackendError> {
    match source {
        MachineSource::Shipped(id) => shipped_machine(id)
            .map(str::to_owned)
            .ok_or_else(|| BackendError::Machine(format!("no shipped machine {id:?}"))),
        MachineSource::User(path) => {
            let bytes = read_limited(path, MAX_MACHINE_FILE_BYTES)?;
            String::from_utf8(bytes).map_err(|_| {
                io_error(
                    path,
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "not UTF-8 text"),
                )
            })
        }
    }
}
