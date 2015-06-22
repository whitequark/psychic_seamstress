extern crate nanovg;
extern crate touptek;

mod image;

pub use ui::image::Image;

pub struct UI<'a> {
    nvg: &'a nanovg::Context,
    pub cam_image: Image<'a>
}

impl<'a> UI<'a> {
    pub fn new(nvg: &nanovg::Context) -> UI {
        UI { nvg: nvg, cam_image: Image::new(nvg) }
    }

    pub fn draw(&self, fb_width: f32, fb_height: f32) {
        let nvg = self.nvg;
        if self.cam_image.present() {
            self.cam_image.draw_to_fit(0.0, 0.0, fb_width, fb_height)
        }
    }
}
