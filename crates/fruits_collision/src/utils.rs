use fruits_ffi::{FfiBox, FfiOption};

use crate::{CollisionAabb, CollisionShape, overlaps};

// todo: recursion to iteration

#[repr(C)]
#[derive(Default, Debug)]
pub struct Bvh<T: Clone> {
    root: FfiOption<BvhNode<T>>,
}

#[repr(C)]
#[derive(Debug)]
struct BvhNode<T: Clone> {
    aabb: CollisionAabb,
    core: BvhNodeCore<T>,
}

#[repr(C)]
#[derive(Debug)]
enum BvhNodeCore<T: Clone> {
    Leaf(CollisionShape, T),
    Branch(FfiBox<[BvhNode<T>; 2]>),
}

impl<T: Clone> Bvh<T> {
    pub fn new(values: impl Iterator<Item = (CollisionShape, T)>) -> Self {
        let mut values = values
            .map(|(s, t)| (s, s.to_aabb(), t))
            .collect::<Vec<_>>();

        Self {
            root: BvhNode::new(&mut values, 0).into(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }
}

impl<T: Clone> Bvh<T> {
    pub fn query(&self, query: CollisionShape, hits: &mut Vec<T>) {
        let Some(root) = self.root.as_ref() else {
            return;
        };

        root.query(query, hits);
    }
}

impl<T: Clone> BvhNode<T> {
    fn new(values: &mut [(CollisionShape, CollisionAabb, T)], depth: usize) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        if values.len() == 1 {
            let (shape, aabb, item) = values[0].clone();

            return Some(Self {
                aabb,
                core: BvhNodeCore::Leaf(shape, item),
            });
        }

        let axis = depth % 3;
        values.sort_unstable_by(|a, b| {
            let va = a.1.center[axis] - a.1.extents[axis];
            let vb = b.1.center[axis] - b.1.extents[axis];

            va.partial_cmp(&vb).unwrap()
        });

        let mid = values.len() / 2;
        let (left, right) = values.split_at_mut(mid);
        let left = BvhNode::new(left, depth + 1).unwrap();
        let right = BvhNode::new(right, depth + 1).unwrap();

        let aabb = left.aabb.merge(right.aabb);

        Some(BvhNode {
            aabb,
            core: BvhNodeCore::Branch(FfiBox::new([left, right])),
        })
    }
}

impl<T: Clone> BvhNode<T> {
    fn query(&self, query: CollisionShape, hits: &mut Vec<T>) {
        if !overlaps(self.aabb.into(), query) {
            return;
        }

        match &self.core {
            BvhNodeCore::Leaf(shape, id) => {
                if overlaps(*shape, query) {
                    hits.push(id.clone());
                }
            }
            BvhNodeCore::Branch(children) => {
                for child in children.iter() {
                    child.query(query, hits);
                }
            }
        }
    }
}
