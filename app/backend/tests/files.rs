//! Reading and saving programs, and listing machine files.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use app_backend::files::{
    list_machine_files, read_limited, read_machine_file, read_program, save_program,
    shipped_machine, MachineSource, SHIPPED_MACHINES,
};
use app_backend::BackendError;

/// A new empty folder for one test, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("editsim-backend-{}-{name}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        Self(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn save_then_read_round_trips_and_replaces() {
    let dir = TempDir::new("roundtrip");
    let path = dir.path("part.nc");
    save_program(&path, b"G0 X1.\n").expect("save");
    save_program(&path, b"G0 X2.\n").expect("save again");
    assert_eq!(read_program(&path).expect("read"), b"G0 X2.\n");
    // No temporary file is left behind.
    let names: Vec<_> = fs::read_dir(&dir.0)
        .expect("list")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(names, vec![std::ffi::OsString::from("part.nc")]);
}

#[test]
fn failed_save_reports_the_path() {
    let dir = TempDir::new("failsave");
    let path = dir.path("missing-folder/part.nc");
    let err = save_program(&path, b"x").expect_err("fails");
    assert!(matches!(&err, BackendError::Io { .. }));
    assert!(err.to_string().contains("part.nc"), "{err}");
}

#[test]
fn missing_file_is_an_error_with_its_path() {
    let dir = TempDir::new("missing");
    let err = read_program(&dir.path("nope.nc")).expect_err("fails");
    assert!(err.to_string().contains("nope.nc"), "{err}");
}

#[test]
fn files_over_the_limit_are_refused() {
    let dir = TempDir::new("limit");
    let path = dir.path("big.nc");
    fs::write(&path, [b'x'; 10]).expect("write");
    assert_eq!(read_limited(&path, 10).expect("at the limit").len(), 10);
    assert!(matches!(
        read_limited(&path, 9),
        Err(BackendError::FileTooLarge { limit: 9, .. })
    ));
}

#[test]
fn shipped_machines_are_listed_first_and_load() {
    let dir = TempDir::new("shipped");
    let list = list_machine_files(&dir.path("no-such-folder")).expect("list");
    assert_eq!(list.len(), SHIPPED_MACHINES.len());
    assert_eq!(list[0].label, "Generic 3-axis VMC");
    assert_eq!(list[1].label, "Generic 3-axis router (no travel limits)");
    assert_eq!(list[2].label, "Generic 2-axis lathe");
    for m in &list {
        assert_eq!(m.problem, None);
        let text = read_machine_file(&m.source).expect("read");
        app_backend::load_machine(&text).expect("loads");
    }
    assert!(shipped_machine("no-such").is_none());
}

#[test]
fn user_machine_files_are_listed_with_their_problems() {
    let dir = TempDir::new("user");
    fs::write(dir.path("b_lathe.toml"), machine_lathe::EXAMPLE_DEFINITION).expect("write");
    fs::write(dir.path("a_broken.toml"), "[machine]\nname = \"x\"\n").expect("write");
    fs::write(dir.path("notes.txt"), "not a machine").expect("write");
    fs::create_dir(dir.path("sub.toml")).expect("a folder named like a file");
    let list = list_machine_files(&dir.0).expect("list");
    let user: Vec<_> = list.iter().skip(SHIPPED_MACHINES.len()).collect();
    assert_eq!(user.len(), 2, "only .toml files: {user:?}");
    assert_eq!(user[0].label, "a_broken.toml");
    assert!(user[0].problem.is_some());
    assert_eq!(user[1].label, "Generic 2-axis lathe");
    assert_eq!(
        user[1].source,
        MachineSource::User(dir.path("b_lathe.toml"))
    );
    assert_eq!(user[1].problem, None);
}
