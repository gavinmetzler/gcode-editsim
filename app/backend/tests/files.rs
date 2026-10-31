//! Reading and saving programs, and listing machine files.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use app_backend::files::{
    list_machine_files, read_limited, read_machine_file, read_program, save_program,
    seed_machine_folder, shipped_machine, MachineSource, SEEDED_MACHINES, SHIPPED_MACHINES,
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
fn shipped_machines_are_the_fallback_when_the_folder_has_none() {
    let dir = TempDir::new("shipped");
    for folder in [dir.path("no-such-folder"), dir.0.clone()] {
        fs::write(dir.path("notes.txt"), "not a machine").expect("write");
        let list = list_machine_files(&folder).expect("list");
        assert_eq!(list.len(), SHIPPED_MACHINES.len());
        assert_eq!(list[0].label, "Generic 3-axis VMC");
        assert_eq!(list[1].label, "Generic 3-axis router (no travel limits)");
        assert_eq!(list[2].label, "Generic 2-axis lathe");
        for m in &list {
            assert_eq!(m.problem, None);
            let text = read_machine_file(&m.source).expect("read");
            app_backend::load_machine(&text).expect("loads");
        }
    }
    assert!(shipped_machine("no-such").is_none());
}

#[test]
fn seeding_copies_the_examples_into_a_new_folder() {
    let dir = TempDir::new("seed");
    let folder = dir.path("config/machines");
    assert!(seed_machine_folder(&folder).expect("seed"));
    for id in SEEDED_MACHINES {
        let text = fs::read_to_string(folder.join(format!("{id}.toml"))).expect("written");
        assert_eq!(Some(text.as_str()), shipped_machine(id));
    }
    // The listing is now the user's files, the mill first so it is the
    // default choice.
    let list = list_machine_files(&folder).expect("list");
    assert_eq!(list.len(), SEEDED_MACHINES.len());
    assert_eq!(list[0].label, "Generic 3-axis VMC");
    assert!(list
        .iter()
        .all(|m| matches!(m.source, MachineSource::User(_)) && m.problem.is_none()));
}

#[test]
fn seeding_leaves_a_folder_with_machine_files_alone() {
    let dir = TempDir::new("seed-kept");
    fs::write(dir.path("mine.toml"), "[machine]\nname = \"x\"\n").expect("write");
    assert!(!seed_machine_folder(&dir.0).expect("seed"));
    let names: Vec<_> = fs::read_dir(&dir.0)
        .expect("list")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(names, vec![std::ffi::OsString::from("mine.toml")]);
}

#[test]
fn seeding_never_overwrites_a_file() {
    // A folder with no .toml files but a same-named folder in the way:
    // seeding writes what it can and reports the rest.
    let dir = TempDir::new("seed-blocked");
    let blocked = format!("{}.toml", SEEDED_MACHINES[0]);
    fs::create_dir(dir.path(&blocked)).expect("folder in the way");
    assert!(seed_machine_folder(&dir.0).is_err());
    assert!(dir.path(&blocked).is_dir());
}

#[test]
fn seeding_fails_cleanly_where_the_folder_cannot_be_made() {
    let dir = TempDir::new("seed-file");
    fs::write(dir.path("machines"), "a file, not a folder").expect("write");
    let err = seed_machine_folder(&dir.path("machines")).expect_err("fails");
    assert!(err.to_string().contains("machines"), "{err}");
}

#[test]
fn user_machine_files_are_listed_with_their_problems() {
    let dir = TempDir::new("user");
    fs::write(dir.path("b_lathe.toml"), machine_lathe::EXAMPLE_DEFINITION).expect("write");
    fs::write(dir.path("a_broken.toml"), "[machine]\nname = \"x\"\n").expect("write");
    fs::write(dir.path("notes.txt"), "not a machine").expect("write");
    fs::create_dir(dir.path("sub.toml")).expect("a folder named like a file");
    let user = list_machine_files(&dir.0).expect("list");
    assert_eq!(user.len(), 2, "only the user's .toml files: {user:?}");
    assert_eq!(user[0].label, "a_broken.toml");
    assert!(user[0].problem.is_some());
    assert_eq!(user[1].label, "Generic 2-axis lathe");
    assert_eq!(
        user[1].source,
        MachineSource::User(dir.path("b_lathe.toml"))
    );
    assert_eq!(user[1].problem, None);
}
