use fruits_engine::{ChildComponent, EntityId, WorldQuery};

pub mod entries;
pub mod serialization;

pub fn subsequence_match_ignore_case(src: &str, pattern: &str) -> bool {
    let mut pattern_chars = pattern.chars().peekable();

    for c in src.chars() {
        let Some(pattern_c) = pattern_chars.peek() else {
            return true;
        };

        if pattern_c.to_ascii_lowercase() == c.to_ascii_lowercase() {
            pattern_chars.next();
        }
    }

    return !pattern_chars.peek().is_some();
}

pub fn find_in_parents<R>(parent_q: WorldQuery<&ChildComponent>, mut child: EntityId, mut finder: impl FnMut(EntityId) -> Option<R>) -> Option<R> {
    loop {
        let Some(ChildComponent { parent }) = parent_q.get(child).copied() else {
            return None;
        };

        if child == parent {
            eprintln!("child has self as parent");
            return None;
        }

        child = parent;

        if let Some(result) = finder(child) {
            return Some(result);
        }
    }
}