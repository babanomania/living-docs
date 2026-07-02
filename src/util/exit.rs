/// Nothing drifted; nothing to do.
pub const OK: i32 = 0;

/// `check` found drift. Not a failure of the tool itself.
// DECISION: unused until Phase 3 wires up `check`; kept here now because
// the full exit-code contract is a §0 invariant, not a per-phase detail.
#[allow(dead_code)]
pub const DRIFT_FOUND: i32 = 1;

/// The tool could not complete the command.
pub const ERROR: i32 = 2;
