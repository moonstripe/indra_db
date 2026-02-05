//! Git-like operations: branch, checkout, diff, merge

mod branch;
mod diff;

pub use branch::{checkout, BranchManager};
pub use diff::{diff_trees, Diff, DiffEntry};
