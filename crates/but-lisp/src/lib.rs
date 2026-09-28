//! A Lisp parser for our diff filtering language.
//!
//! # Select files
//!
//! Changes to one file, Rust source files, or lockfiles:
//!
//! ```lisp
//! (file :path "src/main.rs")
//! (file :glob "src/**/*.rs")
//! (file :extension "lock")
//! ```
//!
//! Added, deleted, renamed, or binary files:
//!
//! ```lisp
//! (file :status :added)
//! (file :status :deleted)
//! (file :status :renamed)
//! (file :binary true)
//! ```
//!
//! # Select lines or hunks
//!
//! Changed TODO lines, only additions, or only removals:
//!
//! ```lisp
//! (line :contains "TODO")
//! (line-added :contains "TODO")
//! (line-removed :contains "TODO")
//! ```
//!
//! Select whole hunks instead. Context lines are not searched:
//!
//! ```lisp
//! (hunk :contains "TODO")
//! (hunk-added :contains "TODO")
//! (hunk-removed :contains "TODO")
//! ```
//!
//! Added logging lines, or additions on new-file lines 10 through 30 (inclusive):
//!
//! ```lisp
//! (line-added :regex "console\\.(log|debug)")
//! (line-added :range '(10 30))
//! ```
//!
//! # Combine selections
//!
//! Source files and tests:
//!
//! ```lisp
//! (union (file :glob "src/**/*.rs") (file :glob "tests/**/*.rs"))
//! ```
//!
//! Only TODO removals in Rust source files:
//!
//! ```lisp
//! (intersection (file :glob "src/**/*.rs") (line-removed :contains "TODO"))
//! ```
//!
//! Everything except added TODO lines:
//!
//! ```lisp
//! (not (line-added :contains "TODO"))
//! ```
//!
//! Rust source changes except whole hunks introducing TODOs:
//!
//! ```lisp
//! (difference (file :glob "src/**/*.rs") (hunk-added :contains "TODO"))
//! ```
//!
//! Also leave added debug statements uncommitted:
//!
//! ```lisp
//! (difference
//!   (file :glob "src/**/*.rs")
//!   (union
//!     (hunk-added :contains "TODO")
//!     (line-added :contains "dbg!")))
//! ```
//!
//! `difference` takes two operands: `(difference a b)` equals `(intersection a (not b))`.
