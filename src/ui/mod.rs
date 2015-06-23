extern crate nanovg;
extern crate touptek;

use std::cell::RefCell;

pub mod geometry;
pub mod image;
pub mod widget;

pub use ui::geometry::{Point, Rect, Direction};
pub use ui::image::Image;
pub use ui::widget::{Widget, Container, Label, Slider, BoxLayout, Frame};

// Overlay

pub struct Overlay<'nvg, 'elt> {
    nvg: &'nvg nanovg::Context,
    pub background: Image<'nvg>,
    pub frames: Vec<Frame<'nvg>>,
    state: RefCell<OverlayState<'elt>>,
}

struct OverlayState<'elt> {
    mouse_at: Point,
    hovered: Option<(&'elt Widget, Point)>,
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
            frame.prepare();

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
        self.nvg.global_alpha(0.8);
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
                if let Some((widget, proj_point)) = frame.project(point) {
                    println!("offset: {:?}", point - proj_point);
                    new_hovered = Some((widget, point - proj_point));
                    break
                }
            }

            match (state.hovered, new_hovered) {
                (None, None) => (),
                (Some((widget, _)), None) => {
                    widget.mouse_out()
                },
                (None, Some((widget, _))) => {
                    widget.mouse_in()
                },
                (Some((old_widget, _)), Some((new_widget, _))) => {
                    if !old_widget.is(new_widget) {
                        old_widget.mouse_out();
                        new_widget.mouse_in()
                    }
                }
            };

            state.hovered = new_hovered
        }

        state.mouse_at = point;
        if let Some((widget, offset)) = state.hovered {
            widget.mouse_move(state.mouse_at - offset);
        }
    }

    pub fn mouse_down(&self) {
        let mut state = self.state.borrow_mut();
        if let Some((widget, offset)) = state.hovered {
            state.captured = true;
            widget.mouse_down(state.mouse_at - offset)
        }
    }

    pub fn mouse_up(&self) {
        let mut state = self.state.borrow_mut();
        if let Some((widget, offset)) = state.hovered {
            state.captured = false;
            widget.mouse_up(state.mouse_at - offset)
        }
    }
}
