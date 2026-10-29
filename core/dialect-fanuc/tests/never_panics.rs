//! The dialect never panics, whatever the input.

mod common;

use common::{axes, run, run_with};
use dialect_fanuc::{DecimalInput, FanucOptions};
use proptest::prelude::*;

fn gcode_like() -> impl Strategy<Value = String> {
    proptest::string::string_regex(
        r"([GMgm](0|1|2|3|4|5|6|8|9|17|18|19|20|21|28|30|40|41|43|49|54|59|80|81|90|91|94|95|98|99)?|[XYZABIJKRFSTPQHDNO][-+]?[0-9]{0,6}\.?[0-9]{0,4}|#1|[ ()\n])*",
    )
    .expect("valid regex")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn arbitrary_gcode_like_text(src in gcode_like()) {
        let _ = run(&src);
    }

    #[test]
    fn arbitrary_text_on_unusual_machines(src in any::<String>(), calculator in any::<bool>()) {
        let options = FanucOptions {
            decimal_input: if calculator { DecimalInput::Calculator } else { DecimalInput::LeastIncrement },
        };
        let none = axes(&[]);
        let _ = run_with(options, &none, &src);
        let odd = axes(&[('U', Some('I'), false), ('C', None, true)]);
        let _ = run_with(options, &odd, &src);
    }
}

#[test]
fn extreme_values() {
    for src in [
        "G0 X99999999999999999999999999999.\n",
        "T99999999999999\n",
        "G99999999999\n",
        "M4294967296\n",
        "G2 R0 X1. F1.\n",
        "F.0000000001 G1 X1.\n",
    ] {
        let _ = run(src);
    }
}
