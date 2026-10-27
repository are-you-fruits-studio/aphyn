//! # fruits_math_macros
//!
//! Compile-time hex color literals: turn a `"#rrggbb"` / `"#rrggbbaa"` string
//! into a fixed-size color array so a wrong color is a build error, not a
//! runtime surprise.
//!
//! # How to use
//!
//! Each macro takes a single string literal and expands, at compile time, to a
//! plain array literal — so the result is a `const`-usable value with no runtime
//! parsing cost. Pick the macro that matches the channel count (RGB vs RGBA) and
//! component type (`u8` 0–255 or `f32` 0.0–1.0) the call site needs. The hash
//! prefix is optional; a malformed literal aborts compilation with a clear
//! message.
//!
//! #### Build an 8-bit RGBA color
//!
//! Parse a `"#rrggbbaa"` literal into `[u8; 4]` for an API that expects four
//! byte channels.
//!
//! ```
//! let color = fruits_math_macros::rgba_u8_array!("#ff8800ff");
//! assert_eq!(color, [255, 136, 0, 255]);
//! ```
//!
//! #### Build a normalized RGB color
//!
//! Parse a `"#rrggbb"` literal into `[f32; 3]`, each channel scaled to the
//! `0.0..=1.0` range a shader or GPU clear color expects.
//!
//! ```
//! let color = fruits_math_macros::rgb_f32_array!("#ff0000");
//! assert_eq!(color, [1.0, 0.0, 0.0]);
//! ```
//!
//! #### The other channel layouts
//!
//! [`rgb_u8_array!`](crate::rgb_u8_array) yields `[u8; 3]` and
//! [`rgba_f32_array!`](crate::rgba_f32_array) yields `[f32; 4]`; both follow the
//! same literal format as the examples above.
//!
//! ```
//! assert_eq!(fruits_math_macros::rgb_u8_array!("#ff8800"), [255, 136, 0]);
//! assert_eq!(fruits_math_macros::rgba_f32_array!("#ff0000ff"), [1.0, 0.0, 0.0, 1.0]);
//! ```
//!
//! # How to maintain
//!
//! The crate is a thin proc-macro front end over [`fruits_math`]'s `const`
//! color parsers: each macro extracts the inner text of the input string literal
//! and forwards it to the matching `fruits_math::parse_color_*` function, then
//! prints the returned array back out as Rust source that re-parses into a
//! `TokenStream`. All real parsing — hex decoding, channel ordering, and the
//! `u8 / 255.0` normalization for the `f32` variants — lives in `fruits_math`;
//! this crate only moves tokens.
//!
//! Two helpers do the token bridging. `to_string_literal` takes the first
//! `TokenTree`, requires it to be a `Literal`, and `to_exact_string_literal`
//! strips the surrounding quotes by hand — it checks the first and last
//! characters are `"` and copies the rest verbatim. It does **not** interpret
//! escape sequences, so the macros assume a simple `"#..."` literal; raw or
//! escaped string literals are not supported. `to_token_stream` renders the
//! parsed array with `{:?}` and re-parses the text, which is why the input must
//! be a single literal token and not, say, a `const` expression.
//!
//! Every failure path — a missing token, a non-literal token, a value
//! `fruits_math` rejects, or a re-parse error — funnels through
//! `panic_invalid_format`, so any misuse surfaces as the same compile-time
//! panic naming the two accepted formats.

use std::fmt::Debug;

use proc_macro::{Literal, TokenStream, TokenTree};

#[proc_macro]
pub fn rgba_u8_array(stream: TokenStream) -> TokenStream {
    to_token_stream(fruits_math::parse_color_rgba_u8(&to_string_literal(stream)).unwrap_or_else(|| panic_invalid_format()))
}

#[proc_macro]
pub fn rgb_u8_array(stream: TokenStream) -> TokenStream {
    to_token_stream(fruits_math::parse_color_rgb_u8(&to_string_literal(stream)).unwrap_or_else(|| panic_invalid_format()))
}

#[proc_macro]
pub fn rgba_f32_array(stream: TokenStream) -> TokenStream {
    to_token_stream(fruits_math::parse_color_rgba_f32(&to_string_literal(stream)).unwrap_or_else(|| panic_invalid_format()))
}

#[proc_macro]
pub fn rgb_f32_array(stream: TokenStream) -> TokenStream {
    to_token_stream(fruits_math::parse_color_rgb_f32(&to_string_literal(stream)).unwrap_or_else(|| panic_invalid_format()))
}

fn to_string_literal(stream: TokenStream) -> String {
    let mut iter = stream.into_iter();

    let literal = iter.next().unwrap_or_else(|| panic_invalid_format());

    let TokenTree::Literal(literal) = literal else {
        panic_invalid_format();
    };

    to_exact_string_literal(literal)
}

fn to_token_stream<T: Debug, const N: usize>(v: [T; N]) -> TokenStream {
    let mut result = String::new();

    result.push_str("[");
    result.push_str(&v.into_iter().map(|x| format!("{:?}", x)).collect::<Vec<_>>().join(", "));
    result.push_str("]");

    result.parse().unwrap_or_else(|_| panic_invalid_format())
}

fn to_exact_string_literal(lit: Literal) -> String {
    let lit = lit.to_string();

    let mut chars = lit.chars();

    let Some('"') = chars.next() else {
        panic_invalid_format();
    };

    let mut result = String::new();

    let mut chars = chars.peekable();

    if chars.peek().is_none() {
        panic_invalid_format();
    }

    loop {
        let c = chars.next().unwrap();

        if chars.peek().is_some() {
            result.push(c);
            continue;
        }

        if c != '"' {
            panic_invalid_format();
        }

        break;
    }

    result
}

fn panic_invalid_format() -> ! {
    panic!("macro expects a single string literal of format \"#ffffffff\" or \"#ffffff\"");
}
