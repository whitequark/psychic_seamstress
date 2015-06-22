extern crate nanovg;
extern crate touptek;

pub struct Image<'a> {
    nvg: &'a nanovg::Context,
    nvg_image: Option<nanovg::Image>,
}

impl<'a> Image<'a> {
    pub fn new(nvg: &nanovg::Context) -> Image {
        Image { nvg: nvg, nvg_image: None }
    }

    pub fn present(&self) -> bool {
        self.nvg_image.is_some()
    }

    pub fn set(&mut self, nvg_image: nanovg::Image) {
        if self.present() { self.unset() }
        self.nvg_image = Some(nvg_image)
    }

    pub fn unset(&mut self) {
        self.nvg.delete_image(self.nvg_image.take().unwrap());
    }

    pub fn from_touptek(&mut self, raw_image: touptek::Image) {
        let touptek::Resolution { width, height } = raw_image.resolution;
        self.set(self.nvg.create_image_rgba(width as i32, height as i32, &raw_image.data).unwrap())
    }

    pub fn draw(&self, left: f32, top: f32, width: f32, height: f32) {
        let nvg_image = self.nvg_image.as_ref().unwrap();
        self.nvg.begin_path();
        self.nvg.rect(left, top, width, height);
        self.nvg.fill_paint(
            self.nvg.image_pattern(left, top, width, height, 0.0,
                                   nvg_image, nanovg::PatternRepeat::NOREPEAT, 1.0));
        self.nvg.fill();
    }

    pub fn draw_unscaled(&self, left: f32, top: f32) {
        let nvg_image = self.nvg_image.as_ref().unwrap();
        let (width, height) = self.nvg.image_size(nvg_image);
        self.draw(left, top, width as f32, height as f32)
    }

    pub fn draw_to_fit(&self, left: f32, top: f32, ext_width: f32, ext_height: f32) {
        fn max(a: f32, b: f32) -> f32 { if a > b { a } else { b }}

        let nvg_image = self.nvg_image.as_ref().unwrap();
        let (int_width, int_height) =
            { let (w,h) = self.nvg.image_size(nvg_image); (w as f32, h as f32) };

        let x_scale = max(int_width, ext_width) / ext_width;
        let y_scale = max(int_height, ext_height) / ext_height;
        if x_scale > y_scale {
            let offset = ext_height - (int_height / x_scale);
            self.draw(left, top + offset / 2.0, int_width / x_scale, int_height / x_scale)
        } else {
            let offset = ext_width - (int_width / y_scale);
            self.draw(left + offset / 2.0, top, int_width / y_scale, int_height / y_scale)
        }
    }
}

impl<'a> Drop for Image<'a> {
    fn drop(&mut self) {
        if self.present() { self.unset() }
    }
}
