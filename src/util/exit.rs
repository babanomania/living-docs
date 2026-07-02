/// Nothing drifted; nothing to do.
pub const OK: i32 = 0;

/// `check` found drift. Not a failure of the tool itself.
pub const DRIFT_FOUND: i32 = 1;

/// The tool could not complete the command.
pub const ERROR: i32 = 2;
