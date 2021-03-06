#![allow(dead_code)]

use mem_arena::MemArena;

use crate::{
    algorithm::partition, bbox::BBox, boundable::Boundable, lerp::lerp_slice, ray::AccelRay,
    timer::Timer,
};

use super::{
    bvh_base::{BVHBase, BVHBaseNode, BVH_MAX_DEPTH},
    ACCEL_NODE_RAY_TESTS, ACCEL_TRAV_TIME,
};

#[derive(Copy, Clone, Debug)]
pub struct BVH<'a> {
    root: Option<&'a BVHNode<'a>>,
    depth: usize,
}

#[derive(Copy, Clone, Debug)]
pub enum BVHNode<'a> {
    Internal {
        bounds_len: u16,
        split_axis: u8,
        bounds_start: &'a BBox,
        children: (&'a BVHNode<'a>, &'a BVHNode<'a>),
    },

    Leaf {
        bounds_start: &'a BBox,
        bounds_len: u16,
        object_range: (usize, usize),
    },
}

impl<'a> BVH<'a> {
    pub fn from_objects<'b, T, F>(
        arena: &'a MemArena,
        objects: &mut [T],
        objects_per_leaf: usize,
        bounder: F,
    ) -> BVH<'a>
    where
        F: 'b + Fn(&T) -> &'b [BBox],
    {
        if objects.is_empty() {
            BVH {
                root: None,
                depth: 0,
            }
        } else {
            let base = BVHBase::from_objects(objects, objects_per_leaf, bounder);

            BVH {
                root: Some(BVH::construct_from_base(
                    arena,
                    &base,
                    base.root_node_index(),
                )),
                depth: base.depth,
            }
        }
    }

    pub fn tree_depth(&self) -> usize {
        self.depth
    }

    pub fn traverse<T, F>(&self, rays: &mut [AccelRay], objects: &[T], mut obj_ray_test: F)
    where
        F: FnMut(&T, &mut [AccelRay]),
    {
        if self.root.is_none() {
            return;
        }

        let mut timer = Timer::new();
        let mut trav_time: f64 = 0.0;
        let mut node_tests: u64 = 0;

        let ray_sign = [
            rays[0].dir_inv.x() >= 0.0,
            rays[0].dir_inv.y() >= 0.0,
            rays[0].dir_inv.z() >= 0.0,
        ];

        // +2 of max depth for root and last child
        let mut node_stack = [self.root.unwrap(); BVH_MAX_DEPTH + 2];
        let mut ray_i_stack = [rays.len(); BVH_MAX_DEPTH + 2];
        let mut stack_ptr = 1;

        while stack_ptr > 0 {
            node_tests += ray_i_stack[stack_ptr] as u64;
            match *node_stack[stack_ptr] {
                BVHNode::Internal {
                    children,
                    bounds_start,
                    bounds_len,
                    split_axis,
                } => {
                    let bounds =
                        unsafe { std::slice::from_raw_parts(bounds_start, bounds_len as usize) };
                    let part = partition(&mut rays[..ray_i_stack[stack_ptr]], |r| {
                        (!r.is_done()) && lerp_slice(bounds, r.time).intersect_accel_ray(r)
                    });
                    if part > 0 {
                        ray_i_stack[stack_ptr] = part;
                        ray_i_stack[stack_ptr + 1] = part;
                        if ray_sign[split_axis as usize] {
                            node_stack[stack_ptr] = children.1;
                            node_stack[stack_ptr + 1] = children.0;
                        } else {
                            node_stack[stack_ptr] = children.0;
                            node_stack[stack_ptr + 1] = children.1;
                        }
                        stack_ptr += 1;
                    } else {
                        stack_ptr -= 1;
                    }
                }

                BVHNode::Leaf {
                    object_range,
                    bounds_start,
                    bounds_len,
                } => {
                    let bounds =
                        unsafe { std::slice::from_raw_parts(bounds_start, bounds_len as usize) };
                    let part = partition(&mut rays[..ray_i_stack[stack_ptr]], |r| {
                        (!r.is_done()) && lerp_slice(bounds, r.time).intersect_accel_ray(r)
                    });

                    trav_time += timer.tick() as f64;

                    if part > 0 {
                        for obj in &objects[object_range.0..object_range.1] {
                            obj_ray_test(obj, &mut rays[..part]);
                        }
                    }

                    timer.tick();

                    stack_ptr -= 1;
                }
            }
        }

        trav_time += timer.tick() as f64;
        ACCEL_TRAV_TIME.with(|att| {
            let v = att.get();
            att.set(v + trav_time);
        });
        ACCEL_NODE_RAY_TESTS.with(|anv| {
            let v = anv.get();
            anv.set(v + node_tests);
        });
    }

    #[allow(clippy::mut_from_ref)]
    fn construct_from_base(
        arena: &'a MemArena,
        base: &BVHBase,
        node_index: usize,
    ) -> &'a mut BVHNode<'a> {
        match base.nodes[node_index] {
            BVHBaseNode::Internal {
                bounds_range,
                children_indices,
                split_axis,
            } => {
                let node = unsafe { arena.alloc_uninitialized_with_alignment::<BVHNode>(32) };

                let bounds = arena
                    .copy_slice_with_alignment(&base.bounds[bounds_range.0..bounds_range.1], 32);
                let child1 = BVH::construct_from_base(arena, base, children_indices.0);
                let child2 = BVH::construct_from_base(arena, base, children_indices.1);

                *node = BVHNode::Internal {
                    bounds_len: bounds.len() as u16,
                    split_axis: split_axis,
                    bounds_start: &bounds[0],
                    children: (child1, child2),
                };

                node
            }

            BVHBaseNode::Leaf {
                bounds_range,
                object_range,
            } => {
                let node = unsafe { arena.alloc_uninitialized::<BVHNode>() };
                let bounds = arena.copy_slice(&base.bounds[bounds_range.0..bounds_range.1]);

                *node = BVHNode::Leaf {
                    bounds_start: &bounds[0],
                    bounds_len: bounds.len() as u16,
                    object_range: object_range,
                };

                node
            }
        }
    }
}

lazy_static! {
    static ref DEGENERATE_BOUNDS: [BBox; 1] = [BBox::new()];
}

impl<'a> Boundable for BVH<'a> {
    fn bounds(&self) -> &[BBox] {
        match self.root {
            None => &DEGENERATE_BOUNDS[..],
            Some(root) => match *root {
                BVHNode::Internal {
                    bounds_start,
                    bounds_len,
                    ..
                }
                | BVHNode::Leaf {
                    bounds_start,
                    bounds_len,
                    ..
                } => unsafe { std::slice::from_raw_parts(bounds_start, bounds_len as usize) },
            },
        }
    }
}
