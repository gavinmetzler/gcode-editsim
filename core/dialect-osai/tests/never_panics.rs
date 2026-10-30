//! The dialect takes untrusted text: it must never panic.

mod common;

use proptest::prelude::*;

proptest! {
    #[test]
    fn arbitrary_text_does_not_panic(src in "[ -~\n]{0,200}") {
        let _ = common::run(&src);
    }

    #[test]
    fn arbitrary_osai_like_blocks_do_not_panic(
        lines in proptest::collection::vec(
            "(G[0-9]{1,3}|M[0-9]{1,2}|[XYZIJKRFST]-?[0-9]{1,5}(\\.[0-9]{0,3})?|\\((UAO|DIS|ROT),[0-9A-Z, ]{0,8}\\)| ){0,8}",
            0..20,
        )
    ) {
        let _ = common::run(&(lines.join("\n") + "\n"));
    }
}
