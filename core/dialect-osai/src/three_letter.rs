//! Three-letter commands such as `(UAO,1)` (manual chapter 1, "Block
//! Types").
//!
//! On a 10 Series control a parenthesis holds a command, not a comment
//! (comments start with `;`). The shared parser reads parentheses as
//! comments, so the dialect finds the commands among a block's
//! parenthesis comments.

/// A three-letter command and its comma-separated arguments, trimmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThreeLetter {
    pub(crate) name: String,
    pub(crate) args: Vec<String>,
}

/// Reads the text between the parentheses: three letters, then optional
/// arguments after commas. `None` if it is not shaped like a command.
pub(crate) fn parse(text: &str) -> Option<ThreeLetter> {
    let mut parts = text.split(',');
    let name = parts.next()?.trim();
    let shaped = name.len() == 3 && name.chars().all(|c| c.is_ascii_alphabetic());
    if !shaped {
        return None;
    }
    Some(ThreeLetter {
        name: name.to_ascii_uppercase(),
        args: parts.map(|a| a.trim().to_owned()).collect(),
    })
}

/// Commands that cannot change the path: messages and the graphics window.
/// Alternative names from manual p. 1-5 are included.
pub(crate) const NO_PATH_EFFECT: &[&str] = &["DIS", "UGS", "UCG", "CGS", "CLG", "DGS", "DCG"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_name_and_arguments() {
        assert_eq!(
            parse("UAO, 2, X, Y"),
            Some(ThreeLetter {
                name: "UAO".into(),
                args: vec!["2".into(), "X".into(), "Y".into()],
            })
        );
        assert_eq!(parse("uao,1").map(|t| t.name), Some("UAO".into()));
        assert_eq!(parse("UDA").map(|t| t.args.len()), Some(0));
    }

    #[test]
    fn other_text_is_not_a_command() {
        for text in ["", "TOOL 1 DRILL", "AB,1", "ABCD,1", "U1O,1", "é€,1"] {
            assert_eq!(parse(text), None, "{text:?}");
        }
    }
}
