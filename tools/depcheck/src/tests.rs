//! Each rule is tested by giving the checker a graph that breaks it.

use super::*;

const ROOT: &str = "/ws";

/// A dependency: `(name, kind, target, dir)`, where `dir` is relative to the
/// root (empty for a registry crate).
type Dep = (
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
    &'static str,
);
/// A package: `(name, dir, dependencies)`.
type Pkg = (&'static str, &'static str, Vec<Dep>);

/// Builds `cargo metadata`-shaped data.
fn metadata(packages: &[Pkg]) -> Metadata {
    let packages: Vec<serde_json::Value> = packages
        .iter()
        .map(|(name, dir, deps)| {
            let deps: Vec<serde_json::Value> = deps
                .iter()
                .map(|(dep, kind, target, dep_dir)| {
                    let path = (!dep_dir.is_empty()).then(|| format!("{ROOT}/{dep_dir}"));
                    serde_json::json!({
                        "name": dep, "kind": kind, "target": target, "path": path,
                    })
                })
                .collect();
            serde_json::json!({
                "name": name,
                "manifest_path": format!("{ROOT}/{dir}/Cargo.toml"),
                "dependencies": deps,
            })
        })
        .collect();
    serde_json::from_value(serde_json::json!({ "packages": packages, "workspace_root": ROOT }))
        .expect("valid metadata")
}

fn rules(allowed: &[(&str, &[&str])]) -> Rules {
    Rules {
        allowed: allowed
            .iter()
            .map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect()))
            .collect(),
        dev: DevRules {
            allowed_dirs: vec!["core".into()],
        },
    }
}

const RULES_SMALL: &[(&str, &[&str])] = &[
    ("ir", &[]),
    ("dialect-api", &["ir"]),
    ("machine-api", &["ir"]),
    ("interpreter", &["ir", "dialect-api", "machine-api"]),
];

fn base_ok() -> Vec<Pkg> {
    vec![
        ("ir", "core/ir", vec![]),
        (
            "dialect-api",
            "core/dialect-api",
            vec![("ir", None, None, "core/ir")],
        ),
        (
            "machine-api",
            "core/machine-api",
            vec![("ir", None, None, "core/ir")],
        ),
        (
            "interpreter",
            "core/interpreter",
            vec![
                ("ir", None, None, "core/ir"),
                ("dialect-api", None, None, "core/dialect-api"),
                ("machine-api", None, None, "core/machine-api"),
            ],
        ),
    ]
}

fn run(packages: &[Pkg], r: &Rules) -> Vec<Violation> {
    check(&metadata(packages), r)
}

#[test]
fn shipped_rules_parse() {
    let r = Rules::parse(RULES).expect("rules.toml is valid");
    assert_eq!(r.allowed.get("ir"), Some(&vec![]));
    assert!(r.allowed.contains_key("interpreter"));
}

#[test]
fn allowed_graph_passes() {
    assert_eq!(run(&base_ok(), &rules(RULES_SMALL)), vec![]);
}

#[test]
fn forbidden_normal_dependency_fails() {
    // The machine side must not depend on the dialect side.
    let mut p = base_ok();
    p[2].2.push(("dialect-api", None, None, "core/dialect-api"));
    let v = run(&p, &rules(RULES_SMALL));
    assert_eq!(
        v,
        vec![Violation::Forbidden {
            krate: "machine-api".into(),
            dependency: "dialect-api".into(),
            kind: "normal".into(),
            allowed: vec!["ir".into()],
        }]
    );
    assert!(v[0].to_string().contains("machine-api may only use: ir"));
}

#[test]
fn ir_may_depend_on_nothing() {
    let mut p = base_ok();
    p[0].2.push(("dialect-api", None, None, "core/dialect-api"));
    let v = run(&p, &rules(RULES_SMALL));
    assert_eq!(v.len(), 1);
    assert!(v[0].to_string().contains("no workspace crates"), "{}", v[0]);
}

#[test]
fn build_and_platform_specific_dependencies_are_checked_too() {
    let mut p = base_ok();
    p[2].2
        .push(("dialect-api", Some("build"), None, "core/dialect-api"));
    p[1].2.push((
        "machine-api",
        None,
        Some("cfg(windows)"),
        "core/machine-api",
    ));
    let v = run(&p, &rules(RULES_SMALL));
    let kinds: Vec<String> = v
        .iter()
        .map(|v| match v {
            Violation::Forbidden { kind, .. } => kind.clone(),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        vec!["normal (cfg(windows))".to_owned(), "build".to_owned()]
    );
}

#[test]
fn dev_dependencies_may_use_core_crates_only() {
    let mut p = base_ok();
    // Tests wiring a concrete crate together is fine...
    p[3].2
        .push(("machine-api", Some("dev"), None, "core/machine-api"));
    assert_eq!(run(&p, &rules(RULES_SMALL)), vec![]);
    // ...but not reaching into app/.
    p.push(("app", "app", vec![]));
    p[0].2.push(("app", Some("dev"), None, "app"));
    let mut r = rules(RULES_SMALL);
    r.allowed.insert("app".into(), vec!["ir".into()]);
    let v = run(&p, &r);
    assert_eq!(
        v,
        vec![Violation::ForbiddenDev {
            krate: "ir".into(),
            dependency: "app".into(),
            location: "app".into(),
        }]
    );
}

#[test]
fn core_may_not_depend_on_app_as_a_normal_dependency() {
    let mut p = base_ok();
    p.push((
        "app",
        "app",
        vec![("interpreter", None, None, "core/interpreter")],
    ));
    p[3].2.push(("app", None, None, "app"));
    let mut r = rules(RULES_SMALL);
    r.allowed.insert("app".into(), vec!["interpreter".into()]);
    let v = run(&p, &r);
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(
        matches!(&v[0], Violation::Forbidden { krate, dependency, .. } if krate == "interpreter" && dependency == "app")
    );
}

#[test]
fn every_workspace_crate_must_be_classified() {
    let mut p = base_ok();
    p.push(("newcrate", "core/newcrate", vec![]));
    let v = run(&p, &rules(RULES_SMALL));
    assert_eq!(
        v,
        vec![Violation::Unclassified {
            krate: "newcrate".into()
        }]
    );
}

#[test]
fn rules_must_not_name_crates_that_do_not_exist() {
    let mut r = rules(RULES_SMALL);
    r.allowed.insert("ghost".into(), vec!["phantom".into()]);
    let v = run(&base_ok(), &r);
    assert!(v.contains(&Violation::UnknownInRules {
        name: "ghost".into()
    }));
    assert!(v.contains(&Violation::UnknownInRules {
        name: "phantom".into()
    }));
}

#[test]
fn path_dependency_outside_the_workspace_fails() {
    // e.g. core reaching into app/ before app joins the workspace.
    let mut p = base_ok();
    p[3].2.push(("app", None, None, "app"));
    let v = run(&p, &rules(RULES_SMALL));
    assert!(
        matches!(&v[..], [Violation::OutsideWorkspace { krate, .. }] if krate == "interpreter"),
        "{v:?}"
    );
}

#[test]
fn registry_crate_with_a_members_name_is_not_a_workspace_dependency() {
    // A crates.io package called "dialect-api" is not our crate.
    let mut p = base_ok();
    p[2].2.push(("dialect-api", None, None, ""));
    assert_eq!(run(&p, &rules(RULES_SMALL)), vec![]);
}

#[test]
fn renamed_dependencies_are_checked_by_real_name() {
    // cargo metadata reports the real package name even when the manifest
    // renames it (`foo = { package = "dialect-api", ... }`).
    let mut p = base_ok();
    p[2].2.push(("dialect-api", None, None, "core/dialect-api"));
    assert_eq!(run(&p, &rules(RULES_SMALL)).len(), 1);
}
