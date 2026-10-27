//! # fruits_control_flow_macros
//!
//! Small `macro_rules!` shorthands for early-exit control flow, used throughout
//! the engine to keep guard clauses in systems and loops to a single line.
//!
//! # How to use
//!
//! Every macro is exported at the crate root by `#[macro_export]`, so reach it
//! through `fruits_control_flow_macros::<name>!`. The two families are: the
//! `*_if!` macros take a boolean condition, and the `*_if_not!` macros take a
//! `let`-else binding and run their exit when the pattern does **not** match.
//!
//! #### Bail out of a function on a condition
//!
//! Skip the rest of a function (typically an ECS system that has nothing to do
//! this frame) when a guard holds, without the `if { return; }` boilerplate.
//!
//! ```
//! fn process(ready: bool, out: &mut i32) {
//!     fruits_control_flow_macros::return_if!(!ready);
//!     *out = 1;
//! }
//!
//! let mut out = 0;
//! process(false, &mut out);
//! assert_eq!(out, 0);
//! process(true, &mut out);
//! assert_eq!(out, 1);
//! ```
//!
//! #### Skip or stop a loop iteration
//!
//! [`continue_if!`](crate::continue_if) skips the current iteration and
//! [`break_if!`](crate::break_if) leaves the loop when the condition holds.
//!
//! ```
//! let mut sum = 0;
//! for n in 0..10 {
//!     fruits_control_flow_macros::continue_if!(n % 2 == 0);
//!     fruits_control_flow_macros::break_if!(n > 5);
//!     sum += n;
//! }
//! assert_eq!(sum, 1 + 3 + 5);
//! ```
//!
//! #### Bind a pattern or bail out
//!
//! The `*_if_not!` macros unwrap a `let`-else binding and exit when the pattern
//! fails — the bound names stay in scope for the rest of the block. This is the
//! common way to pull a value out of an `Option` query result in a system and
//! return early when it is absent.
//!
//! ```
//! fn handle(maybe: Option<i32>, out: &mut i32) {
//!     fruits_control_flow_macros::return_if_not!(Some(value) = maybe);
//!     *out = value;
//! }
//!
//! let mut out = 0;
//! handle(None, &mut out);
//! assert_eq!(out, 0);
//! handle(Some(7), &mut out);
//! assert_eq!(out, 7);
//! ```
//!
//! The loop variants behave the same way, continuing or breaking instead of
//! returning:
//!
//! ```
//! let inputs = [Some(1), None, Some(3)];
//! let mut sum = 0;
//! for item in inputs {
//!     fruits_control_flow_macros::continue_if_not!(Some(n) = item);
//!     sum += n;
//! }
//! assert_eq!(sum, 4);
//! ```
//!
//! # How to maintain
//!
//! Each macro is a single-arm `macro_rules!` that expands to the obvious
//! control-flow statement, so the expansion inherits the diverging keyword's
//! context: `return_if!`/`return_if_not!` require the surrounding function to
//! return `()`, and the `continue_*`/`break_*` variants must sit inside a loop.
//! There is no error reporting beyond the standard "`return` outside of
//! function" / "`break` outside of loop" diagnostics the compiler emits on the
//! expanded code.
//!
//! The two families differ in the fragment they accept. The `*_if!` macros take
//! an `expr` and wrap it in `if $cond { <exit>; }`. The `*_if_not!` macros take
//! a `$p:pat = $e:expr` and expand to a `let`-else (`let $p = $e else { <exit>;
//! };`), so the bindings introduced by the pattern remain visible after the
//! macro — that visibility is the reason these are macros and not functions.

#[macro_export]
macro_rules! return_if {
    ($cond: expr) => {
        if $cond {
            return;
        }
    };
}

#[macro_export]
macro_rules! continue_if {
    ($cond: expr) => {
        if $cond {
            continue;
        }
    };
}

#[macro_export]
macro_rules! break_if {
    ($cond: expr) => {
        if $cond {
            break;
        }
    };
}

#[macro_export]
macro_rules! return_if_not {
    ($p: pat = $e: expr) => {
        let $p = $e else {
            return;
        };
    };
}

#[macro_export]
macro_rules! continue_if_not {
    ($p: pat = $e: expr) => {
        let $p = $e else {
            continue;
        };
    };
}

#[macro_export]
macro_rules! break_if_not {
    ($p: pat = $e: expr) => {
        let $p = $e else {
            break;
        };
    };
}
