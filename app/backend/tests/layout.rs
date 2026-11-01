//! The remembered window layout (M4.2): pane sizes, the maximised pane,
//! and the window's size, place and maximised state.

use app_backend::settings::{
    place_window, MonitorArea, PaneLayout, Placement, Settings, WindowGeometry, DEFAULT_WINDOW,
    MIN_WINDOW,
};

fn geometry(width: f64, height: f64, at: Option<(i32, i32)>, maximized: bool) -> WindowGeometry {
    WindowGeometry {
        width,
        height,
        x: at.map(|p| p.0),
        y: at.map(|p| p.1),
        maximized,
    }
}

/// A 1920 x 1040 work area (a 40 px panel), scale 1, at the origin.
const HD: MonitorArea = MonitorArea {
    x: 0,
    y: 0,
    width: 1920,
    height: 1040,
    scale: 1.0,
};

/// A 4K screen at scale 2 to the right of `HD`: 1920 x 1080 logical.
const UHD_RIGHT: MonitorArea = MonitorArea {
    x: 1920,
    y: 0,
    width: 3840,
    height: 2160,
    scale: 2.0,
};

#[test]
fn the_layout_round_trips_through_the_settings_json() {
    let mut s = Settings::default();
    s.layout = Some(PaneLayout {
        editor: Some(0.35),
        panel: Some(0.25),
        maximised: Some("viewport".to_owned()),
    });
    s.window = Some(geometry(1400.0, 900.0, Some((100, 50)), true));
    let loaded = Settings::from_json(s.to_json().as_bytes());
    assert_eq!(loaded.problem, None);
    assert_eq!(loaded.settings, s);
    let value: serde_json::Value = serde_json::from_str(&s.to_json()).expect("json");
    assert_eq!(
        value["layout"],
        serde_json::json!({ "editor": 0.35, "panel": 0.25, "maximised": "viewport" })
    );
    assert_eq!(
        value["window"],
        serde_json::json!({ "width": 1400.0, "height": 900.0, "x": 100, "y": 50, "maximized": true })
    );
}

#[test]
fn the_machine_and_dialect_survive_a_layout_change() {
    let text = br#"{"version":1,"machine":{"kind":"user","file":"r.toml"},"dialect":"osai"}"#;
    let mut s = Settings::from_json(text).settings;
    s.layout = Some(PaneLayout::default());
    let again = Settings::from_json(s.to_json().as_bytes()).settings;
    assert_eq!(again.dialect.as_deref(), Some("osai"));
    assert!(again.machine.is_some());
}

#[test]
fn shares_out_of_range_are_dropped_and_the_rest_kept() {
    for bad in ["0", "1", "-0.2", "1.5", "\"half\"", "true"] {
        let text = format!(r#"{{"layout": {{"editor": {bad}, "panel": 0.3}}}}"#);
        let loaded = Settings::from_json(text.as_bytes());
        let layout = loaded.settings.layout.expect("layout kept");
        assert_eq!(layout.editor, None, "{bad}");
        assert_eq!(layout.panel, Some(0.3), "{bad}");
        assert!(loaded.problem.is_some(), "{bad}");
    }
}

#[test]
fn a_damaged_layout_or_window_is_dropped_with_a_problem() {
    let loaded = Settings::from_json(br#"{"layout": 3, "window": [1], "dialect": "fanuc"}"#);
    assert_eq!(loaded.settings.layout, None);
    assert_eq!(loaded.settings.window, None);
    assert_eq!(loaded.settings.dialect.as_deref(), Some("fanuc"));
    let problem = loaded.problem.expect("reported");
    assert!(
        problem.contains("layout") && problem.contains("window"),
        "{problem}"
    );

    for bad in [
        r#"{"window": {"width": 0, "height": 800}}"#,
        r#"{"window": {"width": -5, "height": 800}}"#,
        r#"{"window": {"width": 800}}"#,
    ] {
        let loaded = Settings::from_json(bad.as_bytes());
        assert_eq!(loaded.settings.window, None, "{bad}");
        assert!(loaded.problem.is_some(), "{bad}");
    }
}

#[test]
fn a_window_without_a_position_or_maximised_flag_is_fine() {
    let loaded = Settings::from_json(br#"{"window": {"width": 1000, "height": 700}}"#);
    assert_eq!(loaded.problem, None);
    assert_eq!(
        loaded.settings.window,
        Some(geometry(1000.0, 700.0, None, false))
    );
}

#[test]
fn a_window_that_fits_comes_back_as_it_was() {
    let p = place_window(&geometry(1400.0, 900.0, Some((100, 50)), false), &[HD]);
    assert_eq!(
        p,
        Placement {
            width: 1400.0,
            height: 900.0,
            position: Some((100, 50)),
            maximized: false
        }
    );
}

#[test]
fn the_maximised_state_comes_back() {
    let p = place_window(&geometry(1400.0, 900.0, None, true), &[HD]);
    assert!(p.maximized);
    assert_eq!((p.width, p.height), (1400.0, 900.0));
}

#[test]
fn a_window_too_big_for_the_screen_is_shrunk_to_it() {
    // Saved on a big screen, opened on a laptop.
    let laptop = MonitorArea {
        x: 0,
        y: 0,
        width: 1366,
        height: 728,
        scale: 1.0,
    };
    let p = place_window(&geometry(2500.0, 1400.0, Some((10, 10)), false), &[laptop]);
    assert_eq!((p.width, p.height), (1366.0, 728.0));
    assert_eq!(p.position, Some((0, 0)), "moved so it fits");
}

#[test]
fn a_window_partly_off_the_screen_is_moved_back_on() {
    let p = place_window(&geometry(1200.0, 800.0, Some((1500, 600)), false), &[HD]);
    assert_eq!(p.position, Some((720, 240)));
    assert_eq!((p.width, p.height), (1200.0, 800.0));
}

#[test]
fn a_window_on_a_screen_that_is_gone_is_placed_by_the_desktop() {
    // Saved on the right-hand screen, which is now unplugged.
    let p = place_window(&geometry(1200.0, 800.0, Some((2500, 100)), false), &[HD]);
    assert_eq!(p.position, None);
    assert_eq!((p.width, p.height), (1200.0, 800.0));
}

#[test]
fn sizes_are_logical_and_fitted_to_the_screen_the_window_is_on() {
    // On the scale-2 screen: 1920 x 1080 logical, so 1900 x 1000 fits.
    let p = place_window(
        &geometry(1900.0, 1000.0, Some((2000, 100)), false),
        &[HD, UHD_RIGHT],
    );
    assert_eq!((p.width, p.height), (1900.0, 1000.0));
    // 1900 logical = 3800 physical: moved left so it ends at the edge.
    assert_eq!(p.position, Some((1960, 100)));
}

#[test]
fn a_tiny_or_unusable_size_gives_a_usable_one() {
    let p = place_window(&geometry(50.0, 20.0, None, false), &[HD]);
    assert_eq!((p.width, p.height), MIN_WINDOW);
    for bad in [f64::NAN, f64::INFINITY, -1.0] {
        let p = place_window(&geometry(bad, bad, None, false), &[HD]);
        assert_eq!((p.width, p.height), DEFAULT_WINDOW, "{bad}");
    }
}

#[test]
fn without_any_screen_information_the_size_is_kept_and_the_place_left_to_the_desktop() {
    let p = place_window(&geometry(1400.0, 900.0, Some((100, 50)), false), &[]);
    assert_eq!((p.width, p.height), (1400.0, 900.0));
    assert_eq!(p.position, None);
}

#[test]
fn a_screen_smaller_than_the_minimum_gets_the_whole_screen() {
    let small = MonitorArea {
        x: 0,
        y: 0,
        width: 600,
        height: 380,
        scale: 1.0,
    };
    let p = place_window(&geometry(1200.0, 800.0, None, false), &[small]);
    assert_eq!((p.width, p.height), (600.0, 380.0));
}

#[test]
fn a_layout_from_the_ui_is_checked() {
    let good = PaneLayout {
        editor: Some(0.3),
        panel: None,
        maximised: Some("panel".to_owned()),
    };
    assert_eq!(good.clone().checked(), Ok(good));
    for share in [0.0, 1.0, -0.1, 2.0, f64::NAN, f64::INFINITY] {
        let bad = PaneLayout {
            editor: None,
            panel: Some(share),
            maximised: None,
        };
        assert!(bad.checked().is_err(), "{share}");
    }
}
