use std::iter;
use std::cell::UnsafeCell;

use algorithm::partition;
use math::{Matrix4x4, multiply_matrix_slices};
use lerp::lerp_slice;
use assembly::{Assembly, Object, InstanceType};
use ray::{Ray, AccelRay};
use surface::SurfaceIntersection;

pub struct Tracer<'a> {
    root: &'a Assembly,
    rays: UnsafeCell<Vec<AccelRay>>, // Should only be used from trace(), not any other methods
    xform_stack: TransformStack,
    isects: Vec<SurfaceIntersection>,
}

impl<'a> Tracer<'a> {
    pub fn from_assembly(assembly: &'a Assembly) -> Tracer<'a> {
        Tracer {
            root: assembly,
            rays: UnsafeCell::new(Vec::new()),
            xform_stack: TransformStack::new(),
            isects: Vec::new(),
        }
    }

    pub fn trace<'b>(&'b mut self, wrays: &[Ray]) -> &'b [SurfaceIntersection] {
        // Ready the rays
        let rays_ptr = self.rays.get();
        unsafe {
            (*rays_ptr).clear();
            (*rays_ptr).reserve(wrays.len());
            let mut ids = 0..(wrays.len() as u32);
            (*rays_ptr).extend(wrays.iter().map(|wr| AccelRay::new(wr, ids.next().unwrap())));
        }

        // Ready the isects
        self.isects.clear();
        self.isects.reserve(wrays.len());
        self.isects.extend(iter::repeat(SurfaceIntersection::Miss).take(wrays.len()));

        // Start tracing
        let ray_refs = unsafe {
            // IMPORTANT NOTE:
            // We're creating an unsafe non-lifetime-bound slice of self.rays
            // here so that we can pass it to trace_assembly() without
            // conflicting with self.
            // Because of this, it is absolutely CRITICAL that self.rays
            // NOT be used in any other methods.  The rays should only be
            // accessed in other methods via the mutable slice passed directly
            // to them in their function parameters.
            &mut (*rays_ptr)[..]
        };
        self.trace_assembly(self.root, wrays, ray_refs);

        return &self.isects;
    }

    fn trace_assembly<'b>(&'b mut self,
                          assembly: &Assembly,
                          wrays: &[Ray],
                          accel_rays: &mut [AccelRay]) {
        assembly.object_accel.traverse(&mut accel_rays[..], &assembly.instances[..], |inst, rs| {
            // Transform rays if needed
            if let Some((xstart, xend)) = inst.transform_indices {
                // Push transforms to stack
                self.xform_stack.push(&assembly.xforms[xstart..xend]);

                // Do transforms
                let xforms = self.xform_stack.top();
                for ray in &mut rs[..] {
                    let id = ray.id;
                    let t = ray.time;
                    ray.update_from_xformed_world_ray(&wrays[id as usize], &lerp_slice(xforms, t));
                }
            }

            // Trace rays
            {
                // This is kind of weird looking, but what we're doing here is
                // splitting the rays up based on direction if they were
                // transformed, and not splitting them up if they weren't
                // transformed.
                // But to keep the actual tracing code in one place (DRY),
                // we map both cases to an array slice that contains slices of
                // ray arrays.  Gah... that's confusing even when explained.
                // TODO: do this in a way that's less confusing.  Probably split
                // the tracing code out into a trace_instance() method or
                // something.
                let mut tmp = if let Some(_) = inst.transform_indices {
                    split_rays_by_direction(rs)
                } else {
                    [&mut rs[..], &mut [], &mut [], &mut [], &mut [], &mut [], &mut [], &mut []]
                };
                let mut ray_sets = if let Some(_) = inst.transform_indices {
                    &mut tmp[..]
                } else {
                    &mut tmp[..1]
                };

                // Loop through the split ray slices and trace them
                for ray_set in ray_sets.iter_mut().filter(|ray_set| ray_set.len() > 0) {
                    match inst.instance_type {
                        InstanceType::Object => {
                            self.trace_object(&assembly.objects[inst.data_index], wrays, ray_set);
                        }

                        InstanceType::Assembly => {
                            self.trace_assembly(&assembly.assemblies[inst.data_index],
                                                wrays,
                                                ray_set);
                        }
                    }
                }
            }

            // Un-transform rays if needed
            if let Some(_) = inst.transform_indices {
                // Pop transforms off stack
                self.xform_stack.pop();

                // Undo transforms
                let xforms = self.xform_stack.top();
                if xforms.len() > 0 {
                    for ray in &mut rs[..] {
                        let id = ray.id;
                        let t = ray.time;
                        ray.update_from_xformed_world_ray(&wrays[id as usize],
                                                          &lerp_slice(xforms, t));
                    }
                } else {
                    for ray in &mut rs[..] {
                        let id = ray.id;
                        ray.update_from_world_ray(&wrays[id as usize]);
                    }
                }
            }
        });
    }

    fn trace_object<'b>(&'b mut self, obj: &Object, wrays: &[Ray], rays: &mut [AccelRay]) {
        match obj {
            &Object::Surface(ref surface) => {
                surface.intersect_rays(rays, wrays, &mut self.isects, self.xform_stack.top());
            }

            &Object::Light(_) => {
                // TODO
            }
        }
    }
}


fn split_rays_by_direction(rays: &mut [AccelRay]) -> [&mut [AccelRay]; 8] {
    // |   |   |   |   |   |   |   |   |
    //     s1  s2  s3  s4  s5  s6  s7
    let s4 = partition(&mut rays[..], |r| r.dir_inv[0].is_sign_positive());

    let s2 = partition(&mut rays[..s4], |r| r.dir_inv[1].is_sign_positive());
    let s6 = s4 + partition(&mut rays[s4..], |r| r.dir_inv[1].is_sign_positive());

    let s1 = partition(&mut rays[..s2], |r| r.dir_inv[2].is_sign_positive());
    let s3 = s2 + partition(&mut rays[s2..s4], |r| r.dir_inv[2].is_sign_positive());
    let s5 = s4 + partition(&mut rays[s4..s6], |r| r.dir_inv[2].is_sign_positive());
    let s7 = s6 + partition(&mut rays[s6..], |r| r.dir_inv[2].is_sign_positive());

    let (rest, rs7) = rays.split_at_mut(s7);
    let (rest, rs6) = rest.split_at_mut(s6);
    let (rest, rs5) = rest.split_at_mut(s5);
    let (rest, rs4) = rest.split_at_mut(s4);
    let (rest, rs3) = rest.split_at_mut(s3);
    let (rest, rs2) = rest.split_at_mut(s2);
    let (rs0, rs1) = rest.split_at_mut(s1);

    [rs0, rs1, rs2, rs3, rs4, rs5, rs6, rs7]
}


struct TransformStack {
    stack: Vec<Matrix4x4>,
    stack_indices: Vec<usize>,
    scratch_space: Vec<Matrix4x4>,
}

impl TransformStack {
    fn new() -> TransformStack {
        let mut ts = TransformStack {
            stack: Vec::new(),
            stack_indices: Vec::new(),
            scratch_space: Vec::new(),
        };

        ts.stack_indices.push(0);
        ts.stack_indices.push(0);

        ts
    }

    fn push(&mut self, xforms: &[Matrix4x4]) {
        assert!(xforms.len() > 0);

        if self.stack.len() == 0 {
            self.stack.extend(xforms);
        } else {
            let sil = self.stack_indices.len();
            let i1 = self.stack_indices[sil - 2];
            let i2 = self.stack_indices[sil - 1];

            self.scratch_space.clear();
            multiply_matrix_slices(&self.stack[i1..i2], xforms, &mut self.scratch_space);

            self.stack.extend(&self.scratch_space);
        }

        self.stack_indices.push(self.stack.len());
    }

    fn pop(&mut self) {
        assert!(self.stack_indices.len() > 1);

        let sl = self.stack.len();
        let sil = self.stack_indices.len();
        let i1 = self.stack_indices[sil - 2];
        let i2 = self.stack_indices[sil - 1];

        self.stack.truncate(sl - (i2 - i1));
        self.stack_indices.pop();
    }

    fn top<'a>(&'a self) -> &'a [Matrix4x4] {
        let sil = self.stack_indices.len();
        let i1 = self.stack_indices[sil - 2];
        let i2 = self.stack_indices[sil - 1];

        &self.stack[i1..i2]
    }
}
