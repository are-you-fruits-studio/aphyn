use std::collections::VecDeque;

use fruits_ffi::FfiVec;

#[repr(C)]
pub struct OrderGraph {
    directions: FfiVec<FfiVec<u64>>,
    directors_count: FfiVec<u64>,
    initial_nodes: FfiVec<u64>,
}

impl OrderGraph {
    pub fn new(directions: FfiVec<FfiVec<u64>>) -> Option<Self> {
        let mut directors_count = FfiVec::from_vec(vec![0_u64; directions.len() as usize]);

        for (src, dst) in directions.iter().enumerate() {
            for &directed_node in dst.iter() {
                if directed_node == src as u64 {
                    return None;
                }

                directors_count[directed_node] += 1;
            }
            if dst.contains(&(src as u64)) {
                return None;
            }
        }

        let initial_nodes = directors_count
            .iter()
            .enumerate()
            .filter(|(_, c)| **c == 0)
            .map(|(i, _)| i as u64)
            .collect();

        // todo: Add graph validation.

        Some(Self {
            directions,
            directors_count,
            initial_nodes,
        })
    }

    pub fn iter(&self) -> OrderGraphIterator {
        OrderGraphIterator::new(self)
    }
}

// todo: ffi?
pub struct OrderGraphIterator {
    directions: FfiVec<FfiVec<u64>>,
    queue: VecDeque<u64>,
    unvisited_directors_count: FfiVec<u64>,
    processing_count: usize,
}

impl OrderGraphIterator {
    pub fn new(graph: &OrderGraph) -> Self {
        let mut queue = VecDeque::new();

        for initial_node in graph.initial_nodes.iter() {
            queue.push_back(*initial_node);
        }

        let unvisited_directors_count = graph.directors_count.clone();

        Self {
            directions: graph.directions.clone(),
            queue,
            unvisited_directors_count,
            processing_count: 0,
        }
    }

    pub fn start_next(&mut self) -> Option<u64> {
        let node = self.queue.pop_front()?;

        self.processing_count += 1;

        Some(node)
    }

    pub fn end(&mut self, node: u64) {
        self.processing_count -= 1;

        for direction in self.directions[node as u64].iter() {
            let direction_directors_count = &mut self.unvisited_directors_count[*direction];

            *direction_directors_count -= 1;

            if *direction_directors_count == 0 {
                self.queue.push_back(*direction);
            }
        }
    }

    pub fn all_started(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn all_ended(&self) -> bool {
        self.all_started() && self.processing_count == 0
    }
}
