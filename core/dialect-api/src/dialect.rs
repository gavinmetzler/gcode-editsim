//! The dialect trait.

use gcode_syntax::{Block, Number, NumberStyle};
use ir::{Diagnostics, LengthUnit, SourceRef};

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

/// What a number in a word measures, for reading and writing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueKind {
    /// A length (a linear axis, an arc offset, a radius): mm inside, the
    /// program's units in the text.
    Length,
    /// An angle (a rotary axis): degrees, whatever the program's units.
    Angle,
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

    /// Why a code transformation (M4.6) cannot rewrite `block`, if it
    /// cannot: codes whose effect is not carried by its axis words, such
    /// as coordinate shifts, canned cycles, machine-coordinate moves and
    /// reference returns. `state` is the modal state after the block. The
    /// message names the code. The default is `None` (a dialect that does
    /// not rewrite numbers is refused anyway).
    fn transform_refusal(&self, block: &Block, state: &ModalState) -> Option<String> {
        let _ = (block, state);
        None
    }

    /// The value of a number literal as this controller reads it, in
    /// internal units (mm or degrees), with the program in `units`. Used to
    /// check rewritten text (M4.4). The default is `None`: a dialect that
    /// does not support rewriting.
    fn read_value(&self, number: &Number, kind: ValueKind, units: LengthUnit) -> Option<f64> {
        let _ = (number, kind, units);
        None
    }

    /// `value` (internal units: mm or degrees) written as this controller
    /// reads it in `units`, in the style of the literal it replaces (or of
    /// one the dialect chooses, for an added word). Reading the result back
    /// with [`Dialect::read_value`] must give `value` within half of the
    /// controller's least input increment. `None` if the value cannot be
    /// written (not finite), and by default: a dialect that does not
    /// support rewriting.
    fn format_value(
        &self,
        value: f64,
        kind: ValueKind,
        units: LengthUnit,
        style: &NumberStyle,
    ) -> Option<String> {
        let _ = (value, kind, units, style);
        None
    }
}
