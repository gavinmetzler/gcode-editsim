//! A machine whose axes the dialect cannot use is reported, not a crash.

use dialect_fanuc::Fanuc;
use interpreter::{interpret, Options};
use ir::{DiagCode, Pos, WorkOffsetId};
use machine_api::{AxisDef, AxisKind, LimitViolation, Machine, SpeedLimits, ToolPose};

struct Broken {
    axes: Vec<AxisDef>,
    speeds: SpeedLimits,
}

impl Machine for Broken {
    fn name(&self) -> &str {
        "broken"
    }
    fn axes(&self) -> &[AxisDef] {
        &self.axes
    }
    fn forward_kinematics(&self, _pos: &Pos) -> ToolPose {
        ToolPose {
            position: [0.0; 3],
            direction: [0.0, 0.0, 1.0],
        }
    }
    fn check_limits(&self, _pos: &Pos) -> Vec<LimitViolation> {
        Vec::new()
    }
    fn work_offset(&self, _id: WorkOffsetId) -> Option<Pos> {
        None
    }
    fn speeds(&self) -> &SpeedLimits {
        &self.speeds
    }
}

#[test]
fn duplicate_axis_letters_give_a_diagnostic() {
    let axis = AxisDef {
        name: 'X',
        kind: AxisKind::Linear,
        travel: None,
        arc_offset_letter: None,
        direction: None,
        rapid_rate: None,
        diameter_mode_capable: false,
    };
    let machine = Broken {
        axes: vec![axis.clone(), axis],
        speeds: SpeedLimits::default(),
    };
    let p = interpret(
        b"G0 X1.\n",
        "t.nc",
        &Fanuc::new().unwrap(),
        &machine,
        Options::default(),
    );
    assert_eq!(p.diagnostics.len(), 1);
    assert_eq!(p.diagnostics[0].code, DiagCode::InvalidValue);
    assert!(p.channels[0].segments.is_empty());
}
