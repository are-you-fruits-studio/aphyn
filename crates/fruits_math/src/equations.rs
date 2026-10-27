/// Solves equation of type ax + b = 0
pub fn eq_linear(a: f32, b: f32) -> LinearEquationResult {
    if a != 0.0 {
        return LinearEquationResult::Success(-b / a);
    }

    match b {
        0.0 => LinearEquationResult::Any,
        _ => LinearEquationResult::None,
    }
}

/// Solves equation of type ax² + bx + c = 0
pub fn eq_quadratic(a: f32, b: f32, c: f32) -> QuadraticEquationResult {
    if a == 0.0 && b == 0.0 {
        return match c {
            0.0 => QuadraticEquationResult::Any,
            _ => QuadraticEquationResult::None,
        };
    }

    let d = b * b - 4.0 * a * c;

    if d < 0.0 {
        return QuadraticEquationResult::None;
    }

    let d_sqrt = match d {
        0.0 => 0.0,
        _ => d.sqrt(),
    };

    if d == 0.0 {
        return QuadraticEquationResult::Single(-b / (2.0 * a));
    }

    let x1 = (-b - d_sqrt) / (2.0 * a);
    let x2 = (-b + d_sqrt) / (2.0 * a);

    QuadraticEquationResult::Double([x1, x2])
}

pub enum LinearEquationResult {
    None,
    Any,
    Success(f32),
}

pub enum QuadraticEquationResult {
    None,
    Any,
    Single(f32),
    Double([f32; 2]),
}
