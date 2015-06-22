#![allow(dead_code)]

extern crate nanovg;

use std::cell::RefCell;

use ui::{Point, Rect, Direction};

// Generic

pub trait Widget {
    fn is(&self, other: &Widget) -> bool {
        (self as *const _ as *const ()) == (other as *const _ as *const ())
    }

    fn size(&self) -> Point;
    fn set_size(&self, size: Point);
    fn size_request(&self) -> Point;

    fn need_reflow(&self) -> bool {
        let Point(rw, rh) = self.size_request();
        let Point(aw, ah) = self.size();
        rw > aw || rh > ah
    }

    fn render(&self);

    fn hover(&self, _point: Point) -> Option<&Widget> { None }
    fn mouse_move(&self, _point: Point) {}
    fn mouse_down(&self, _point: Point) {}
    fn mouse_up(&self, _point: Point) {}
    fn mouse_in(&self) {}
    fn mouse_out(&self) {}
}

pub trait Container<'nvg> {
    fn add(&mut self, widget: Box<Widget + 'nvg>);
    fn remove(&mut self, widget: &Widget) -> Box<Widget + 'nvg>;
    fn iter<'a>(&'a self) -> Iter<'a>;
}

pub struct Iter<'a> {
    elements: &'a Vec<Box<Widget>>,
    index: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Widget;

    fn next(&mut self) -> Option<&'a Widget> {
        self.elements.get(self.index).map(|elem| { self.index += 1; &**elem })
    }
}

// Style

pub struct Style {
    // Fonts
    font_face: &'static str,
    font_size: f32,

    // Colors
    active_color: nanovg::Color,
    hover_color: nanovg::Color,
    passive_color: nanovg::Color,
    background_color: nanovg::Color,

    // Sizes
    line_size: f32,
    frame_corner_size: f32,
}

impl Style {
    fn get() -> &'static Style {
        static STYLE: Style = Style {
            font_face: "Roboto",
            font_size: 32.,
            active_color: nanovg::Color::rgb_f(1., 1., 1.),
            hover_color: nanovg::Color::rgb_f(1., 0.5, 0.),
            passive_color: nanovg::Color::rgb_f(0.5, 0.5, 0.5),
            background_color: nanovg::Color::rgb_f(0.15, 0.15, 0.15),
            line_size: 4.,
            frame_corner_size: 10.,
        };

        return &STYLE;
    }
}

// Label

pub struct Label<'nvg> {
    nvg: &'nvg nanovg::Context,
    state: RefCell<LabelState>,
}

struct LabelState {
    text: String,
    size: Point,
}

impl<'nvg> Label<'nvg> {
    pub fn new(nvg: &'nvg nanovg::Context, text: &str) -> Label<'nvg> {
        Label {
            nvg: nvg,
            state: RefCell::new(LabelState {
                text: String::from(text),
                size: Point(0., 0.),
            })
        }
    }
}

impl<'nvg> Widget for Label<'nvg> {
    fn size(&self) -> Point { self.state.borrow().size }
    fn set_size(&self, size: Point) { self.state.borrow_mut().size = size }

    fn size_request(&self) -> Point {
        self.nvg.font_face(&Style::get().font_face);
        self.nvg.font_size(Style::get().font_size);

        let mut bounds = [0.; 4];
        self.nvg.text_bounds(0., 0., &self.state.borrow().text, &mut bounds);

        Point(bounds[2] - bounds[0], bounds[3] - bounds[1])
    }

    fn render(&self) {
        let state = self.state.borrow();
        self.nvg.font_face(&Style::get().font_face);
        self.nvg.font_size(Style::get().font_size);
        self.nvg.fill_color(Style::get().active_color);
        self.nvg.text_align(nanovg::LEFT | nanovg::TOP);
        self.nvg.text(0., 0., &state.text);
    }
}

// BoxLayout

pub struct BoxLayout<'nvg> {
    nvg: &'nvg nanovg::Context,
    direction: Direction,
    children: Vec<Box<Widget + 'nvg>>,
    state: RefCell<BoxLayoutState>,
}

struct BoxLayoutState {
    size: Point,
}

impl<'nvg> BoxLayout<'nvg> {
    pub fn new(nvg: &'nvg nanovg::Context, dir: Direction) -> BoxLayout<'nvg> {
        BoxLayout {
            nvg: nvg,
            direction: dir,
            children: Vec::new(),
            state: RefCell::new(BoxLayoutState {
                size: Point(0., 0.),
            })
        }
    }

    pub fn horz(nvg: &'nvg nanovg::Context) -> BoxLayout {
        BoxLayout::new(nvg, Direction::Horizontal)
    }

    pub fn vert(nvg: &'nvg nanovg::Context) -> BoxLayout {
        BoxLayout::new(nvg, Direction::Vertical)
    }
}

impl<'nvg> Widget for BoxLayout<'nvg> {
    fn size(&self) -> Point { self.state.borrow().size }

    fn set_size(&self, size: Point) {
        self.state.borrow_mut().size = size;

        let request = self.size_request();
        for child in &self.children {
            match self.direction {
                Direction::Horizontal => {
                    let child_width = child.size_request().0 * size.0 / request.0;
                    child.set_size(Point(child_width, size.1));
                },
                Direction::Vertical => {
                    let child_height = child.size_request().1 * size.1 / request.1;
                    child.set_size(Point(size.0, child_height));
                }
            }
        }
    }

    fn size_request(&self) -> Point {
        let requests = self.children.iter().
            map(|child| { child.size_request() }).collect::<Vec<_>>();
        let xs = requests.iter().map(|req| { req.0 });
        let ys = requests.iter().map(|req| { req.1 });

        match self.direction {
            Direction::Horizontal =>
                Point(xs.sum(), ys.fold(0., |l, r| { l.max(r) })),
            Direction::Vertical =>
                Point(xs.fold(0., |l, r| { l.max(r) }), ys.sum())
        }
    }

    fn need_reflow(&self) -> bool {
        self.children.iter().fold(false, |acc, child| { acc || child.need_reflow() })
    }

    fn render(&self) {
        let (mut x, mut y) = (0., 0.);
        for child in &self.children {
            let Point(w, h) = child.size();

            self.nvg.save();
            self.nvg.translate(x, y);
            self.nvg.scissor(0., 0., w, h);
            child.render();
            self.nvg.restore();

            match self.direction {
                Direction::Horizontal => x += w,
                Direction::Vertical   => y += h
            }
        }
    }

    fn hover(&self, point: Point) -> Option<&Widget> {
        let mut origin = Point(0., 0.);
        for child in &self.children {
            let size = child.size();

            if Rect(origin, size).contains(point) {
                return child.hover(point - origin)
            }

            match self.direction {
                Direction::Horizontal => origin.0 += size.0,
                Direction::Vertical   => origin.1 += size.1
            }
        }

        None
    }

    fn mouse_move(&self, point: Point) {
        let mut origin = Point(0., 0.);
        for child in &self.children {
            let size = child.size();

            if Rect(origin, size).contains(point) {
                return child.mouse_move(point - origin)
            }

            match self.direction {
                Direction::Horizontal => origin.0 += size.0,
                Direction::Vertical   => origin.1 += size.1
            }
        }
    }
}

impl<'nvg> Container<'nvg> for BoxLayout<'nvg> {
    fn add(&mut self, widget: Box<Widget + 'nvg>) {
        self.children.push(widget)
    }

    fn remove(&mut self, widget: &Widget) -> Box<Widget + 'nvg> {
        let index = self.iter().position(|elem| { elem.is(widget) });
        self.children.remove(index.unwrap())
    }

    fn iter<'a>(&'a self) -> Iter<'a> {
        Iter { elements: &self.children, index: 0 }
    }
}

// Frame

pub struct Frame<'nvg> {
    nvg: &'nvg nanovg::Context,
    widget: Box<Widget + 'nvg>,
    state: RefCell<FrameState>,
}

struct FrameState {
    size: Point,
    position: Point,
    moving: Option<Point>,
}

impl<'nvg> Frame<'nvg> {
    pub fn new(nvg: &'nvg nanovg::Context, widget: Box<Widget + 'nvg>) -> Frame<'nvg> {
        Frame {
            nvg: nvg,
            widget: widget,
            state: RefCell::new(FrameState {
                size: Point(0., 0.),
                position: Point(0., 0.),
                moving: None,
            })
        }
    }

    pub fn position(&self) -> Point {
        self.state.borrow().position
    }

    pub fn set_position(&mut self, point: Point) {
        self.state.borrow_mut().position = point
    }

    fn content_offset() -> Point {
        Point(Style::get().frame_corner_size, Style::get().frame_corner_size)
    }
}

impl<'nvg> Widget for Frame<'nvg> {
    fn size(&self) -> Point { self.state.borrow().size }

    fn set_size(&self, size: Point) {
        self.state.borrow_mut().size = size;
        self.widget.set_size(size - Frame::content_offset() * 2.)
    }

    fn size_request(&self) -> Point {
        self.widget.size_request() + Frame::content_offset() * 2.
    }

    fn need_reflow(&self) -> bool { self.widget.need_reflow() }

    fn render(&self) {
        let state = self.state.borrow();
        let (Point(x, y), Point(w, h)) = (state.position, state.size);
        let style = Style::get();

        self.nvg.begin_path();
        self.nvg.rounded_rect(x, y, w, h, style.frame_corner_size);
        self.nvg.stroke_width(style.line_size);
        self.nvg.stroke_color(style.passive_color);
        self.nvg.stroke();
        self.nvg.fill_color(style.background_color);
        self.nvg.fill();

        self.nvg.save();
        self.nvg.translate(x + Style::get().frame_corner_size,
                           y + Style::get().frame_corner_size);
        self.nvg.scissor(0., 0., w, h);
        self.widget.render();
        self.nvg.restore();
    }

    fn hover(&self, point: Point) -> Option<&Widget> {
        let state = self.state.borrow();
        let origin = state.position + Frame::content_offset();

        if Rect(origin, self.widget.size()).contains(point) {
            match self.widget.hover(point - origin) {
                Some(widget) => Some(widget),
                None => Some(self)
            }
        } else if Rect(state.position, self.size()).contains(point) {
            Some(self)
        } else {
            None
        }
    }

    fn mouse_down(&self, point: Point) {
        let mut state = self.state.borrow_mut();
        state.moving = Some(point - state.position)
    }

    fn mouse_up(&self, _point: Point) {
        self.state.borrow_mut().moving = None
    }

    fn mouse_move(&self, point: Point) {
        let mut state = self.state.borrow_mut();
        match state.moving {
            Some(origin) => state.position = (point - origin).round(),
            None => self.widget.mouse_move(point)
        }
    }
}