#![allow(dead_code)]

use std::ops::{Add, Sub, Mul};
use std::cmp::PartialEq;

use lerp::Lerp;
use float4::Float4;

use super::Vector;
use super::Matrix4x4;

/// A position in 3d homogeneous space.
#[derive(Debug, Copy, Clone)]
pub struct Point {
    pub co: Float4,
}

impl Point {
    pub fn new(x: f32, y: f32, z: f32) -> Point {
        Point { co: Float4::new(x, y, z, 1.0) }
    }

    /// Returns the point in standardized coordinates, where the
    /// fourth homogeneous component has been normalized to 1.0.
    pub fn norm(&self) -> Point {
        Point { co: self.co / self.co.get_3() }
    }

    pub fn min(&self, other: Point) -> Point {
        let n1 = self.norm();
        let n2 = other.norm();

        Point { co: n1.co.v_min(n2.co) }
    }

    pub fn max(&self, other: Point) -> Point {
        let n1 = self.norm();
        let n2 = other.norm();

        Point { co: n1.co.v_max(n2.co) }
    }

    pub fn into_vector(self) -> Vector {
        Vector::new(self.co.get_0(), self.co.get_1(), self.co.get_2())
    }

    pub fn get_n(&self, n: usize) -> f32 {
        match n {
            0 => self.x(),
            1 => self.y(),
            2 => self.z(),
            _ => panic!("Attempt to access dimension beyond z."),
        }
    }

    pub fn x(&self) -> f32 {
        self.co.get_0()
    }

    pub fn y(&self) -> f32 {
        self.co.get_1()
    }

    pub fn z(&self) -> f32 {
        self.co.get_2()
    }

    pub fn set_x(&mut self, x: f32) {
        self.co.set_0(x);
    }

    pub fn set_y(&mut self, y: f32) {
        self.co.set_1(y);
    }

    pub fn set_z(&mut self, z: f32) {
        self.co.set_2(z);
    }
}


impl PartialEq for Point {
    fn eq(&self, other: &Point) -> bool {
        self.co == other.co
    }
}


impl Add<Vector> for Point {
    type Output = Point;

    fn add(self, other: Vector) -> Point {
        Point { co: self.co + other.co }
    }
}


impl Sub for Point {
    type Output = Vector;

    fn sub(self, other: Point) -> Vector {
        Vector { co: self.norm().co - other.norm().co }
    }
}

impl Sub<Vector> for Point {
    type Output = Point;

    fn sub(self, other: Vector) -> Point {
        Point { co: self.co - other.co }
    }
}

impl Mul<Matrix4x4> for Point {
    type Output = Point;

    fn mul(self, other: Matrix4x4) -> Point {
        Point {
            co: Float4::new((self.co * other[0]).h_sum(),
                            (self.co * other[1]).h_sum(),
                            (self.co * other[2]).h_sum(),
                            (self.co * other[3]).h_sum()),
        }
    }
}


impl Lerp for Point {
    fn lerp(self, other: Point, alpha: f32) -> Point {
        let s = self.norm();
        let o = other.norm();
        Point { co: (s.co * (1.0 - alpha)) + (o.co * alpha) }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{Vector, Matrix4x4};
    use lerp::Lerp;

    #[test]
    fn norm() {
        let mut p1 = Point::new(1.0, 2.0, 3.0);
        let p2 = Point::new(2.0, 4.0, 6.0);
        p1.co.set_3(0.5);

        assert_eq!(p2, p1.norm());
    }

    #[test]
    fn add() {
        let p1 = Point::new(1.0, 2.0, 3.0);
        let v1 = Vector::new(1.5, 4.5, 2.5);
        let p2 = Point::new(2.5, 6.5, 5.5);

        assert_eq!(p2, p1 + v1);
    }

    #[test]
    fn sub() {
        let p1 = Point::new(1.0, 2.0, 3.0);
        let p2 = Point::new(1.5, 4.5, 2.5);
        let v1 = Vector::new(-0.5, -2.5, 0.5);

        assert_eq!(v1, p1 - p2);
    }

    #[test]
    fn mul_matrix_1() {
        let p = Point::new(1.0, 2.5, 4.0);
        let m = Matrix4x4::new_from_values(1.0,
                                           2.0,
                                           2.0,
                                           1.5,
                                           3.0,
                                           6.0,
                                           7.0,
                                           8.0,
                                           9.0,
                                           2.0,
                                           11.0,
                                           12.0,
                                           0.0,
                                           0.0,
                                           0.0,
                                           1.0);
        let pm = Point::new(15.5, 54.0, 70.0);
        assert_eq!(p * m, pm);
    }

    #[test]
    fn mul_matrix_2() {
        let p = Point::new(1.0, 2.5, 4.0);
        let m = Matrix4x4::new_from_values(1.0,
                                           2.0,
                                           2.0,
                                           1.5,
                                           3.0,
                                           6.0,
                                           7.0,
                                           8.0,
                                           9.0,
                                           2.0,
                                           11.0,
                                           12.0,
                                           2.0,
                                           3.0,
                                           1.0,
                                           5.0);
        let mut pm = Point::new(15.5, 54.0, 70.0);
        pm.co.set_3(18.5);
        assert_eq!(p * m, pm);
    }

    #[test]
    fn mul_matrix_3() {
        // Make sure matrix multiplication composes the way one would expect
        let p = Point::new(1.0, 2.5, 4.0);
        let m1 = Matrix4x4::new_from_values(1.0,
                                            2.0,
                                            2.0,
                                            1.5,
                                            3.0,
                                            6.0,
                                            7.0,
                                            8.0,
                                            9.0,
                                            2.0,
                                            11.0,
                                            12.0,
                                            13.0,
                                            7.0,
                                            15.0,
                                            3.0);
        let m2 = Matrix4x4::new_from_values(4.0,
                                            1.0,
                                            2.0,
                                            3.5,
                                            3.0,
                                            6.0,
                                            5.0,
                                            2.0,
                                            2.0,
                                            2.0,
                                            4.0,
                                            12.0,
                                            5.0,
                                            7.0,
                                            8.0,
                                            11.0);
        println!("{:?}", m1 * m2);

        let pmm1 = p * (m1 * m2);
        let pmm2 = (p * m1) * m2;

        assert!((pmm1 - pmm2).length2() <= 0.00001); // Assert pmm1 and pmm2 are roughly equal
    }

    #[test]
    fn lerp1() {
        let p1 = Point::new(1.0, 2.0, 1.0);
        let p2 = Point::new(-2.0, 1.0, -1.0);
        let p3 = Point::new(1.0, 2.0, 1.0);

        assert_eq!(p3, p1.lerp(p2, 0.0));
    }

    #[test]
    fn lerp2() {
        let p1 = Point::new(1.0, 2.0, 1.0);
        let p2 = Point::new(-2.0, 1.0, -1.0);
        let p3 = Point::new(-2.0, 1.0, -1.0);

        assert_eq!(p3, p1.lerp(p2, 1.0));
    }

    #[test]
    fn lerp3() {
        let p1 = Point::new(1.0, 2.0, 1.0);
        let p2 = Point::new(-2.0, 1.0, -1.0);
        let p3 = Point::new(-0.5, 1.5, 0.0);

        assert_eq!(p3, p1.lerp(p2, 0.5));
    }
}
