#![allow(dead_code)]

use glam::Vec4Mask;

use crate::math::{Matrix4x4, Point, Vector};

type RayIndexType = u16;
type FlagType = u8;
const OCCLUSION_FLAG: FlagType = 1;
const DONE_FLAG: FlagType = 1 << 1;

/// This is never used directly in ray tracing--it's only used as a convenience
/// for filling the RayBatch structure.
#[derive(Debug, Copy, Clone)]
pub struct Ray {
    pub orig: Point,
    pub dir: Vector,
    pub time: f32,
    pub wavelength: f32,
    pub max_t: f32,
}

/// The hot (frequently accessed) parts of ray data.
#[derive(Debug, Copy, Clone)]
struct RayHot {
    orig_local: Point,     // Local-space ray origin
    dir_inv_local: Vector, // Local-space 1.0/ray direction
    max_t: f32,
    time: f32,
    flags: FlagType,
}

/// The cold (infrequently accessed) parts of ray data.
#[derive(Debug, Copy, Clone)]
struct RayCold {
    orig: Point, // World-space ray origin
    dir: Vector, // World-space ray direction
    wavelength: f32,
}

/// A batch of rays, separated into hot and cold parts.
#[derive(Debug)]
pub struct RayBatch {
    hot: Vec<RayHot>,
    cold: Vec<RayCold>,
}

impl RayBatch {
    /// Creates a new empty ray batch.
    pub fn new() -> RayBatch {
        RayBatch {
            hot: Vec::new(),
            cold: Vec::new(),
        }
    }

    /// Creates a new empty ray batch, with pre-allocated capacity for
    /// `n` rays.
    pub fn with_capacity(n: usize) -> RayBatch {
        RayBatch {
            hot: Vec::with_capacity(n),
            cold: Vec::with_capacity(n),
        }
    }

    pub fn push(&mut self, ray: Ray, is_occlusion: bool) {
        self.hot.push(RayHot {
            orig_local: ray.orig,   // Bogus, to place-hold.
            dir_inv_local: ray.dir, // Bogus, to place-hold.
            max_t: ray.max_t,
            time: ray.time,
            flags: if is_occlusion { OCCLUSION_FLAG } else { 0 },
        });
        self.cold.push(RayCold {
            orig: ray.orig,
            dir: ray.dir,
            wavelength: ray.wavelength,
        });
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        self.hot.swap(a, b);
        self.cold.swap(a, b);
    }

    pub fn set_from_ray(&mut self, ray: &Ray, is_occlusion: bool, idx: usize) {
        self.hot[idx].orig_local = ray.orig;
        self.hot[idx].dir_inv_local = Vector {
            co: ray.dir.co.reciprocal(),
        };
        self.hot[idx].max_t = ray.max_t;
        self.hot[idx].time = ray.time;
        self.hot[idx].flags = if is_occlusion { OCCLUSION_FLAG } else { 0 };

        self.cold[idx].orig = ray.orig;
        self.cold[idx].dir = ray.dir;
        self.cold[idx].wavelength = ray.wavelength;
    }

    pub fn truncate(&mut self, len: usize) {
        self.hot.truncate(len);
        self.cold.truncate(len);
    }

    /// Clear all rays, settings the size of the batch back to zero.
    ///
    /// Capacity is maintained.
    pub fn clear(&mut self) {
        self.hot.clear();
        self.cold.clear();
    }

    pub fn len(&self) -> usize {
        self.hot.len()
    }

    /// Updates the accel data of the given ray (at index `idx`) with the
    /// given world-to-local-space transform matrix.
    ///
    /// This should be called when entering (and exiting) traversal of a
    /// new transform space.
    pub fn update_local(&mut self, idx: usize, xform: &Matrix4x4) {
        self.hot[idx].orig_local = self.cold[idx].orig * *xform;
        self.hot[idx].dir_inv_local = Vector {
            co: (self.cold[idx].dir * *xform).co.reciprocal(),
        };
    }

    //==========================================================
    // Data access

    #[inline(always)]
    pub fn orig(&self, idx: usize) -> Point {
        self.cold[idx].orig
    }

    #[inline(always)]
    pub fn dir(&self, idx: usize) -> Vector {
        self.cold[idx].dir
    }

    #[inline(always)]
    pub fn orig_local(&self, idx: usize) -> Point {
        self.hot[idx].orig_local
    }

    #[inline(always)]
    pub fn dir_inv_local(&self, idx: usize) -> Vector {
        self.hot[idx].dir_inv_local
    }

    #[inline(always)]
    pub fn time(&self, idx: usize) -> f32 {
        self.hot[idx].time
    }

    #[inline(always)]
    pub fn max_t(&self, idx: usize) -> f32 {
        self.hot[idx].max_t
    }

    #[inline(always)]
    pub fn set_max_t(&mut self, idx: usize, new_max_t: f32) {
        self.hot[idx].max_t = new_max_t;
    }

    #[inline(always)]
    pub fn wavelength(&self, idx: usize) -> f32 {
        self.cold[idx].wavelength
    }

    /// Returns whether the given ray (at index `idx`) is an occlusion ray.
    #[inline(always)]
    pub fn is_occlusion(&self, idx: usize) -> bool {
        (self.hot[idx].flags & OCCLUSION_FLAG) != 0
    }

    /// Returns whether the given ray (at index `idx`) has finished traversal.
    #[inline(always)]
    pub fn is_done(&self, idx: usize) -> bool {
        (self.hot[idx].flags & DONE_FLAG) != 0
    }

    /// Marks the given ray (at index `idx`) as an occlusion ray.
    #[inline(always)]
    pub fn mark_occlusion(&mut self, idx: usize) {
        self.hot[idx].flags |= OCCLUSION_FLAG
    }

    /// Marks the given ray (at index `idx`) as having finished traversal.
    #[inline(always)]
    pub fn mark_done(&mut self, idx: usize) {
        self.hot[idx].flags |= DONE_FLAG
    }
}

/// A structure used for tracking traversal of a ray batch through a scene.
#[derive(Debug)]
pub struct RayStack {
    lanes: Vec<Lane>,
    tasks: Vec<RayTask>,
}

impl RayStack {
    pub fn new() -> RayStack {
        RayStack {
            lanes: Vec::new(),
            tasks: Vec::new(),
        }
    }

    /// Returns whether the stack is empty of tasks or not.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Makes sure there are at least `count` lanes.
    pub fn ensure_lane_count(&mut self, count: usize) {
        while self.lanes.len() < count {
            self.lanes.push(Lane {
                idxs: Vec::new(),
                end_len: 0,
            })
        }
    }

    pub fn ray_count_in_next_task(&self) -> usize {
        let task = self.tasks.last().unwrap();
        let end = self.lanes[task.lane].end_len;
        end - task.start_idx
    }

    pub fn next_task_ray_idx(&self, i: usize) -> usize {
        let task = self.tasks.last().unwrap();
        let i = i + task.start_idx;
        debug_assert!(i < self.lanes[task.lane].end_len);
        self.lanes[task.lane].idxs[i] as usize
    }

    /// Clears the lanes and tasks of the RayStack.
    ///
    /// Note: this is (importantly) different than calling clear individually
    /// on the `lanes` and `tasks` members.  Specifically, we don't want to
    /// clear `lanes` itself, as that would also free all the memory of the
    /// individual lanes.  Instead, we want to iterate over the individual
    /// lanes and clear them, but leave `lanes` itself untouched.
    pub fn clear(&mut self) {
        for lane in self.lanes.iter_mut() {
            lane.idxs.clear();
            lane.end_len = 0;
        }

        self.tasks.clear();
    }

    /// Pushes the given ray index onto the end of the specified lane.
    pub fn push_ray_index(&mut self, ray_idx: usize, lane: usize) {
        assert!(self.lanes.len() > lane);
        self.lanes[lane].idxs.push(ray_idx as RayIndexType);
    }

    /// Pushes any excess indices on the given lane to a new task on the
    /// task stack.
    ///
    /// Returns whether a task was pushed or not.  No task will be pushed
    /// if there are no excess indices on the end of the lane.
    pub fn push_lane_to_task(&mut self, lane_idx: usize) -> bool {
        if self.lanes[lane_idx].end_len < self.lanes[lane_idx].idxs.len() {
            self.tasks.push(RayTask {
                lane: lane_idx,
                start_idx: self.lanes[lane_idx].end_len,
            });
            self.lanes[lane_idx].end_len = self.lanes[lane_idx].idxs.len();
            true
        } else {
            false
        }
    }

    /// Takes the given list of lane indices, and pushes any excess indices on
    /// the end of each into a new task, in the order provided.
    pub fn push_lanes_to_tasks(&mut self, lane_idxs: &[usize]) {
        for &l in lane_idxs {
            self.push_lane_to_task(l);
        }
    }

    pub fn duplicate_next_task(&mut self) {
        let task = self.tasks.last().unwrap();
        let l = task.lane;
        let start = task.start_idx;
        let end = self.lanes[l].end_len;

        // Extend the indices vector
        self.lanes[l].idxs.reserve(end - start);
        let old_len = self.lanes[l].idxs.len();
        let new_len = old_len + end - start;
        unsafe {
            self.lanes[l].idxs.set_len(new_len);
        }

        // Copy elements
        copy_in_place::copy_in_place(&mut self.lanes[l].idxs, start..end, end);

        // Push the new task onto the stack
        self.tasks.push(RayTask {
            lane: l,
            start_idx: end,
        });

        self.lanes[l].end_len = self.lanes[l].idxs.len();
    }

    // Pops the next task off the stack.
    pub fn pop_task(&mut self) {
        let task = self.tasks.pop().unwrap();
        self.lanes[task.lane].end_len = task.start_idx;
        self.lanes[task.lane].idxs.truncate(task.start_idx);
    }

    // Executes a task without popping it from the task stack.
    pub fn do_next_task<F>(&mut self, mut handle_ray: F)
    where
        F: FnMut(usize),
    {
        let task = self.tasks.last().unwrap();
        let task_range = (task.start_idx, self.lanes[task.lane].end_len);

        // Execute task.
        for i in task_range.0..task_range.1 {
            let ray_idx = self.lanes[task.lane].idxs[i];
            handle_ray(ray_idx as usize);
        }
    }

    /// Pops the next task off the stack, and executes the provided closure for
    /// each ray index in the task.
    #[inline(always)]
    pub fn pop_do_next_task<F>(&mut self, handle_ray: F)
    where
        F: FnMut(usize),
    {
        self.do_next_task(handle_ray);
        self.pop_task();
    }

    /// Pops the next task off the stack, executes the provided closure for
    /// each ray index in the task, and pushes the ray indices back onto the
    /// indicated lanes.
    pub fn pop_do_next_task_and_push_rays<F>(&mut self, output_lane_count: usize, mut handle_ray: F)
    where
        F: FnMut(usize) -> Vec4Mask,
    {
        // Pop the task and do necessary bookkeeping.
        let task = self.tasks.pop().unwrap();
        let task_range = (task.start_idx, self.lanes[task.lane].end_len);
        self.lanes[task.lane].end_len = task.start_idx;

        // SAFETY: this is probably evil, and depends on behavior of Vec that
        // are not actually promised.  But we're essentially truncating the lane
        // to the start of our task range, but will continue to access it's
        // elements beyond that range via `get_unchecked()` below.  Because the
        // memory is not freed nor altered, this is safe.  However, again, the
        // Vec apis don't promise this behavior.  So:
        //
        // TODO: build a slightly different lane abstraction to get this same
        // efficiency without depending on implicit Vec behavior.
        unsafe {
            self.lanes[task.lane].idxs.set_len(task.start_idx);
        }

        // Execute task.
        for i in task_range.0..task_range.1 {
            let ray_idx = *unsafe { self.lanes[task.lane].idxs.get_unchecked(i) };
            let push_mask = handle_ray(ray_idx as usize).bitmask();
            for l in 0..output_lane_count {
                if (push_mask & (1 << l)) != 0 {
                    self.lanes[l as usize].idxs.push(ray_idx);
                }
            }
        }
    }
}

/// A lane within a RayStack.
#[derive(Debug)]
struct Lane {
    idxs: Vec<RayIndexType>,
    end_len: usize,
}

/// A task within a RayStack.
//
// Specifies the lane that the relevant ray pointers are in, and the
// starting index within that lane.  The relevant pointers are always
// `&[start_idx..]` within the given lane.
#[derive(Debug)]
struct RayTask {
    lane: usize,
    start_idx: usize,
}
