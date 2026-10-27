pub fn normalize_serialization_path(path: &str) -> &str {
    path.trim_matches('/')
}

pub fn decompose_serialization_path(mut path: &str) -> Option<(&str, &str)> {
    path = normalize_serialization_path(path);

    if path.is_empty() {
        return None;
    }

    let Some(start_idx) = path.find('/') else {
        return Some((path, ""));
    };

    return Some((&path[..start_idx], normalize_serialization_path(&path[start_idx..])));
}