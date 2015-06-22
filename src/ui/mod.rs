extern crate nanovg;
extern crate touptek;

use std::cell::RefCell;
use std::ops::{Add, Mul, Sub};

pub mod image;
pub mod widget;

pub use ui::image::Image;
pub use ui::widget::{Widget, Container, Label, BoxLayout, Frame};

// Geometry

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

// Overlay

pub struct Overlay<'nvg, 'elt> {
    nvg: &'nvg nanovg::Context,
    pub background: Image<'nvg>,
    pub frames: Vec<Frame<'nvg>>,
    state: RefCell<OverlayState<'elt>>,
}

struct OverlayState<'elt> {
    mouse_at: Point,
    hovered: Option<&'elt Widget>,
    captured: bool,
}

impl<'nvg, 'elt> Overlay<'nvg, 'elt> {
    pub fn new(nvg: &'nvg nanovg::Context) -> Overlay<'nvg, 'elt> {
        Overlay {
            nvg: nvg,
            background: Image::new(nvg),
            frames: vec![],
            state: RefCell::new(OverlayState {
                mouse_at: Point(0., 0.),
                hovered: None,
                captured: false,
            }),
        }
    }

    pub fn prepare(&self) {
        for frame in &self.frames {
            if frame.need_reflow() {
                let size = frame.size_request();
                frame.set_size(size);
            }
        }
    }

    pub fn draw(&self, size: Point) {
        if self.background.present() {
            self.background.draw_to_fit(size.as_rect())
        }

        self.nvg.save();
        self.nvg.global_alpha(0.7);
        for frame in &self.frames {
            frame.render()
        }
        self.nvg.restore();
    }

    pub fn mouse_move(&'elt self, point: Point) {
        let mut state = self.state.borrow_mut();

        if !state.captured {
            let mut new_hovered = None;
            for frame in &self.frames {
                if let Some(hovered) = frame.hover(point) {
                    new_hovered = Some(hovered);
                    break
                }
            }

            match (state.hovered, new_hovered) {
                (None, None) => (),
                (Some(widget), None) => {
                    widget.mouse_out()
                },
                (None, Some(widget)) => {
                    widget.mouse_in()
                },
                (Some(old_widget), Some(new_widget)) => {
                    if !old_widget.is(new_widget) {
                        old_widget.mouse_out();
                        new_widget.mouse_in()
                    }
                }
            };
            state.hovered = new_hovered
        }

        state.mouse_at = point;
        if let Some(widget) = state.hovered {
            widget.mouse_move(state.mouse_at);
        }
    }

    pub fn mouse_down(&self) {
        let mut state = self.state.borrow_mut();
        if let Some(widget) = state.hovered {
            state.captured = true;
            widget.mouse_down(state.mouse_at)
        }
    }

    pub fn mouse_up(&self) {
        let mut state = self.state.borrow_mut();
        if let Some(widget) = state.hovered {
            state.captured = false;
            widget.mouse_up(state.mouse_at)
        }
    }
}
