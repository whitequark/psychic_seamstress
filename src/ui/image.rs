#![allow(dead_code)]

extern crate nanovg;
extern crate touptek;
extern crate png;

use std::cell::RefCell;

use ui::{Point, Rect};

pub struct Image<'a> {
    nvg: &'a nanovg::Context,
    nvg_image: RefCell<Option<nanovg::Image>>,
}

impl<'a> Image<'a> {
    pub fn new(nvg: &nanovg::Context) -> Image {
        Image { nvg: nvg, nvg_image: RefCell::new(None) }
    }

    pub fn present(&self) -> bool {
        self.nvg_image.borrow().is_some()
    }

    pub fn set(&self, nvg_image: nanovg::Image) {
        if self.present() { self.unset() }
        *self.nvg_image.borrow_mut() = Some(nvg_image)
    }

    pub fn unset(&self) {
        self.nvg.delete_image(self.nvg_image.borrow_mut().take().unwrap());
    }

    pub fn from_touptek(&self, mut raw_image: touptek::Image) {
        let touptek::Resolution { width, height } = raw_image.resolution;
        self.set(self.nvg.create_image_rgba(width, height, &raw_image.data).unwrap());
        unsafe { raw_image.data.set_len(0) } // O(1) drop at -O1
    }

    pub fn from_png(&self, raw_image: png::Image) {
        match raw_image.pixels {
            png::PixelsByColorType::RGBA8(ref data) =>
                self.set(self.nvg.create_image_rgba(
                            raw_image.width, raw_image.height, data).unwrap()),
            _ => panic!("unsupported non-RGBA png format")
        }
    }

    pub fn draw(&self, rect: Rect) {
        let nvg_image = self.nvg_image.borrow();
        let nvg_image = nvg_image.as_ref().unwrap();

        let Rect(Point(left, top), Point(width, height)) = rect;
        self.nvg.begin_path();
        self.nvg.rect(left, top, width, height);
        self.nvg.fill_paint(
            self.nvg.image_pattern(left, top, width, height, 0.0,
                                   nvg_image, nanovg::PatternRepeat::NOREPEAT, 1.0));
        self.nvg.fill();
    }

    pub fn draw_unscaled(&self, pos: Point) {
        let nvg_image = self.nvg_image.borrow();
        let nvg_image = nvg_image.as_ref().unwrap();

        let (width, height) = self.nvg.image_size(nvg_image);
        self.draw(Rect(pos, Point(width as f32, height as f32)))
    }

    pub fn draw_to_fit(&self, rect: Rect) {
        let nvg_image = self.nvg_image.borrow();
        let nvg_image = nvg_image.as_ref().unwrap();

        let Rect(Point(left, top), Point(ext_width, ext_height)) = rect;
        let (int_width, int_height) = self.nvg.image_size(nvg_image);
        let (int_width, int_height) = (int_width as f32, int_height as f32);

        let x_scale = int_width.max(ext_width) / ext_width;
        let y_scale = int_height.max(ext_height) / ext_height;
        if x_scale > y_scale {
            let offset = ext_height - (int_height / x_scale);
            self.draw(Rect(Point(left, top + offset / 2.0),
                           Point(int_width / x_scale, int_height / x_scale)))
        } else {
            let offset = ext_width - (int_width / y_scale);
            self.draw(Rect(Point(left + offset / 2.0, top),
                           Point(int_width / y_scale, int_height / y_scale)))
        }
    }
}

impl<'a> Drop for Image<'a> {
    fn drop(&mut self) {
        if self.present() { self.unset() }
    }
}
