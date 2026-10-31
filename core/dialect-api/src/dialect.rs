//! The dialect trait.

use gcode_syntax::Block;
use ir::{Diagnostics, SourceRef};

use crate::axes::AxisSet;
use crate::command::Command;
use crate::state::ModalState;
use crate::table::CodeTable;

/// What a dialect needs to know about the block it is interpreting, beyond
/// the block itself.
#[derive(Debug, Clone, Copy)]
pub struct BlockContext<'a> {
    /// The machine's axes. Dialects look axis letters up here rather than
    /// assuming any exist.
    pub axes: &'a AxisSet,
    /// Source location of the block, for diagnostics.
    pub src: SourceRef,
}

/// Turns parsed blocks into [`Command`]s according to one controller's
/// rules.
pub trait Dialect {
    /// Stable identifier, e.g. the `id` from the code table.
    fn id(&self) -> &str;

    /// The table of codes this dialect supports.
    fn code_table(&self) -> &CodeTable;

    /// The modal state the controller starts in.
    fn initial_state(&self, axes: &AxisSet) -> ModalState;

    /// Interprets one block: updates `state`, appends the block's commands
    /// to `out` in the order they take effect, and reports problems to
    /// `diag`.
    ///
    /// Must not panic on any input. Anything the dialect does not support
    /// must produce a diagnostic, never be silently ignored.
    fn interpret_block(
        &self,
        block: &Block,
        ctx: &BlockContext<'_>,
        state: &mut ModalState,
        out: &mut Vec<Command>,
        diag: &mut Diagnostics,
    );

    /// The program number if `block` starts a program (Fanuc `O1000` or
    /// `:1000`), so the interpreter can find the target of a
    /// [`Command::CallSubprogram`] in the same file. The default is
    /// `None`: a dialect without subprograms never starts one.
    fn program_number(&self, block: &Block) -> Option<u32> {
        let _ = block;
        None
    }
}
