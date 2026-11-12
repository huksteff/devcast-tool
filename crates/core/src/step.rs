//! One registry command by name with its arguments as on the command line (`--arg value`): a
//! line of `devcast`, a step of a plan file, an edit of an inspector panel. `dc-ops` builds and
//! runs it; it lives here so the panels of the effects (below `dc-ops`) can ask for it.

use serde_json::{Map, Value};

use crate::command::Op;

/// The name of a registry command taken from its type: a button asks for `OpName::of::<ZoomSet>()`,
/// not for `"zoom set"`, so a renamed or removed command fails the build, not the click
/// (verify: `op_names`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpName(&'static str);

impl OpName {
    pub const fn of<O: Op>() -> Self {
        Self(O::NAME)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub op: String,
    pub args: Map<String, Value>,
}

impl Step {
    /// A command named by text: a line of `devcast`, a step of a plan file. The code of the
    /// editor and of the effects names a command by its type ([`Step::of`]).
    pub fn new(op: impl Into<String>, args: Map<String, Value>) -> Self {
        Self {
            op: op.into(),
            args,
        }
    }

    pub fn of(op: OpName, args: Map<String, Value>) -> Self {
        Self::new(op.as_str(), args)
    }
}
