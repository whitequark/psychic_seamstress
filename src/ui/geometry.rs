use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Point(pub f32, pub f32);

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rect(pub Point, pub Point);

impl Add for Point {
    type Output = Point;
    fn add(self, rhs: Point) -> Point { Point(self.0 + rhs.0, self.1 + rhs.1) }
}

impl Sub for Point {
    type Output = Point;
    fn sub(self, rhs: Point) -> Point { Point(self.0 - rhs.0, self.1 - rhs.1) }
}

impl Mul<f32> for Point {
    type Output = Point;
    fn mul(self, rhs: f32) -> Point { Point(self.0 * rhs, self.1 * rhs) }
}

impl Point {
    pub fn round(self) -> Point {
        Point(self.0.round(), self.1.round())
    }

    pub fn as_rect(self) -> Rect {
        Rect(Point(0., 0.), self)
    }
}

impl Rect {
    pub fn contains(self, point: Point) -> bool {
        let Rect(Point(l, t), Point(w, h)) = self;
        let Point(x, y) = point;
        x >= l && y >= t && x <= l + w && y <= t + h
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Horizontal,
    Vertical
}
