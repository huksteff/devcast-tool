//! One registry command by name with its arguments as on the command line (`--arg value`): a
//! line of `devcast`, a step of a plan file, an edit of an inspector panel. `dc-ops` builds and
//! runs it; it lives here so the panels of the effects (below `dc-ops`) can ask for it.

use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub op: String,
    pub args: Map<String, Value>,
}

impl Step {
    pub fn new(op: impl Into<String>, args: Map<String, Value>) -> Self {
        Self {
            op: op.into(),
            args,
        }
    }
}
