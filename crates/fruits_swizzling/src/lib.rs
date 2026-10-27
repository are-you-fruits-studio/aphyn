//! # fruits_swizzling
//!
//! Provides the proc-macro that generates the swizzle accessor methods for the
//! engine's small vector types, so the large, mechanical set of
//! component-reordering and resizing methods is produced from a single rule
//! instead of written by hand.
//!
//! # How to use
//!
//! The macro is a build-time code generator. The engine invokes it once in
//! `fruits_math` after the vector types are defined; you normally consume its
//! output through `fruits_math`'s `Vec2`/`Vec3`/`Vec4` rather than expanding it
//! yourself.
//!
//! #### Generate the accessors for a set of vectors
//!
//! Define `Vec2`/`Vec3`/`Vec4` (each with a `new` constructor and `x`/`y`/`z`/`w`
//! fields), then expand [`swizzling!`](crate::swizzling) in the same scope to
//! emit every reorder and resize as inherent methods.
//!
//! ```
//! # struct Vec2<T> { x: T, y: T }
//! # struct Vec3<T> { x: T, y: T, z: T }
//! # struct Vec4<T> { x: T, y: T, z: T, w: T }
//! # impl<T> Vec2<T> { fn new(x: T, y: T) -> Self { Self { x, y } } }
//! # impl<T> Vec3<T> { fn new(x: T, y: T, z: T) -> Self { Self { x, y, z } } }
//! # impl<T> Vec4<T> { fn new(x: T, y: T, z: T, w: T) -> Self { Self { x, y, z, w } } }
//! fruits_swizzling::swizzling! {}
//!
//! // Reorder a vector by naming its components in the desired order.
//! let v = Vec2::new(1, 2);
//! let flipped = v.yx();
//! assert_eq!((flipped.x, flipped.y), (2, 1));
//! ```
//!
//! #### Widen a vector with a fill slot
//!
//! A slot named `n` is not read from the source vector; it becomes an extra
//! parameter, so an accessor like `xyn` resizes a `Vec2` to a `Vec3` by appending
//! the supplied value.
//!
//! ```
//! # struct Vec2<T> { x: T, y: T }
//! # struct Vec3<T> { x: T, y: T, z: T }
//! # struct Vec4<T> { x: T, y: T, z: T, w: T }
//! # impl<T> Vec2<T> { fn new(x: T, y: T) -> Self { Self { x, y } } }
//! # impl<T> Vec3<T> { fn new(x: T, y: T, z: T) -> Self { Self { x, y, z } } }
//! # impl<T> Vec4<T> { fn new(x: T, y: T, z: T, w: T) -> Self { Self { x, y, z, w } } }
//! # fruits_swizzling::swizzling! {}
//! let v = Vec2::new(1, 2);
//! let widened = v.xyn(3);
//! assert_eq!((widened.x, widened.y, widened.z), (1, 2, 3));
//! ```
//!
//! # How to maintain
//!
//! [`swizzling!`](crate::swizzling) ignores its input and returns a freshly built
//! `String` parsed into a `TokenStream` — the code is assembled as text with
//! `format!`, not with `quote!`. It walks every `(input, output)` pair drawn from
//! `{2, 3, 4}` and emits one `impl<T: Copy> Vec{input}<T>` block per pair, so the
//! same accessor set is shared across all three vector types.
//!
//! Each output slot is one symbol over `0..=parameters_count`: the first
//! `parameters_count` symbols select a source component (`x`/`y`/`z`/`w`), and the
//! extra symbol is the fill slot rendered as `n`. `generate_swizzlings_indices`
//! builds the full Cartesian product of these symbols at the output length;
//! `generate_swizzlings_with_zeros` maps the extra symbol to `None`. The all-`n`
//! combination is skipped. A method name is the slot letters concatenated, and the
//! returned vector is `Vec{output}::new(..)` filled with `self.<component>` for
//! real slots and a parameter for each fill slot.
//!
//! Fill parameters are produced by `to_additional_parameters`: a `None` at output
//! position `p` adds a parameter named `PARAMETERS[p]` (so the names follow output
//! position, e.g. `nxy` takes an `x: T`). The additional parameters and the
//! constructor arguments are both emitted in output-position order, which is what
//! keeps them aligned.
//!
//! The `T: Copy` bound is uniform because every accessor copies components by value
//! (`self.x`). Pure permutations would need only `T`, which is the open `// todo:
//! Separate <T: Copy> and <T> swizzling.` at the top of the file; a commented-out
//! `HashSet`-based variant of the index generator is also kept for reference.
//!
//! The macro depends on its expansion site providing `Vec2`/`Vec3`/`Vec4` with a
//! matching `new` and accessible `x`/`y`/`z`/`w` fields; it does not import or
//! define them. The engine satisfies this contract in `fruits_math`'s `vec.rs`,
//! where `swizzling!{}` is expanded right after the `vec_impl!` macro defines the
//! vectors.

use proc_macro::TokenStream;

// todo: Separate <T: Copy> and <T> swizzling.

#[proc_macro]
pub fn swizzling(_item: TokenStream) -> TokenStream {
    let mut result = String::new();

    for swizzle in generate_swizzlings() {
        result.push_str(&swizzle);
        result.push('\n');
    }

    result.parse().unwrap()
}

const PARAMETERS: &[&'static str] = &["x", "y", "z", "w"];
const ZERO_PARAMETER_NAME: &'static str = "n";

fn generate_swizzlings() -> Vec<String> {
    let mut results = Vec::new();

    let counts = [2, 3, 4];

    for input in counts {
        for output in counts {
            results.push(generate_vector_swizzling_impl(input, output));
        }
    }

    results
}

fn generate_vector_swizzling_impl(input_count: usize, output_count: usize) -> String {
    let mut result = String::new();
    result.push_str(&format!("impl<T: Copy> Vec{input_count}<T> {{\n"));

    for s in generate_vector_swizzling(output_count, &PARAMETERS[0..input_count]) {
        result.push_str(&format!("    {}\n", &s));
    }
    result.push_str("}\n");

    result
}

fn generate_vector_swizzling(output_parameters_count: usize, parameters: &[&str]) -> Vec<String> {
    let swizzling_indices = generate_swizzlings_with_zeros(output_parameters_count, parameters.len());

    let lower = parameters.iter().map(|p| p.to_lowercase()).collect::<Vec<_>>();

    let mut results = Vec::<String>::new();

    let zero_name = ZERO_PARAMETER_NAME;

    for indices in swizzling_indices {
        if indices.iter().all(|i| i.is_none()) {
            continue;
        }

        let name = indices
            .iter()
            .map(|i| {
                if let Some(j) = i {
                    lower[*j].clone()
                } else {
                    String::from(zero_name)
                }
            })
            .collect::<Vec<_>>()
            .join("");

        let constructor_arguments = indices
            .iter()
            .enumerate()
            .map(|(position, i)| {
                if let Some(j) = i {
                    format!("self.{}", lower[*j])
                } else {
                    String::from(PARAMETERS[position])
                }
            })
            .collect::<Vec<_>>()
            .join(", ");

        let additional_parameters = to_additional_parameters(indices);

        results.push(format!("pub fn {name}(&self{additional_parameters}) -> Vec{output_parameters_count}<T> {{ Vec{output_parameters_count}::new({constructor_arguments}) }}"));
    }

    results
}

fn generate_swizzlings_with_zeros(output_parameters_count: usize, parameters_count: usize) -> Vec<Vec<Option<usize>>> {
    let mut result = Vec::new();

    for swizzlings_indices in generate_swizzlings_indices(output_parameters_count, parameters_count + 1) {
        result.push(
            swizzlings_indices
                .iter()
                .map(|i| if *i == parameters_count { None } else { Some(*i) })
                .collect::<Vec<_>>(),
        );
    }

    result
}

fn generate_swizzlings_indices(output_parameters_count: usize, parameters_count: usize) -> Vec<Vec<usize>> {
    let mut swizzlings = Vec::new();

    for i in 0..parameters_count {
        swizzlings.push(vec![i]);
    }

    for _ in 1..output_parameters_count {
        let last_swizzlings = swizzlings.clone();
        swizzlings.clear();

        for indices in last_swizzlings {
            for j in 0..parameters_count {
                swizzlings.push(indices.iter().copied().chain(std::iter::once(j)).collect::<Vec<_>>());
            }
        }
    }

    swizzlings
}

// fn GenerateSwizzlingsIndices(outputParametersCount: usize, parametersCount: usize) -> HashSet<Vec<usize>> {
//     let mut swizzlings = HashSet::new();

//     for i in 0..parametersCount {
//         swizzlings.insert(vec![i]);
//     }

//     for i in 1..outputParametersCount {
//         let s = swizzlings.clone();
//         for indices in s {
//             swizzlings.remove(indices);

//             for j in 0..parametersCount {
//                 swizzlings.insert(std::iter::once(j).chain(indices.iter().copied()).collect::<Vec<_>>());
//             }
//         }
//     }

//     swizzlings
// }

fn to_additional_parameters(indices: Vec<Option<usize>>) -> String {
    let mut result = String::new();

    for position in 0..indices.len() {
        if indices[position].is_none() {
            result.push_str(", ");
            result.push_str(PARAMETERS[position]);
            result.push_str(": T");
        }
    }

    result
}
