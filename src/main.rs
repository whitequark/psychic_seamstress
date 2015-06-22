#![allow(unused_unsafe)]

extern crate glfw;
extern crate gl;
extern crate nanovg;
extern crate touptek;

use glfw::Context as GlfwContext;
use nanovg::Context as NvgContext;

mod ui;

macro_rules! gl {
    ($e: expr) => ({
        use gl::*;
        unsafe { $e };
        assert_eq!(unsafe { GetError() }, 0);
    })
}

fn init_gl() {
    gl!(FrontFace(CCW));
    gl!(CullFace(BACK));
    gl!(Enable(CULL_FACE));
    gl!(Enable(BLEND));
    gl!(BlendFunc(SRC_ALPHA, ONE_MINUS_SRC_ALPHA));
    gl!(Enable(SCISSOR_TEST));
}

fn set_alpha(rgba: &mut Vec<u8>, alpha: u8) {
    for i in 0..rgba.len() / 4 {
        unsafe { *rgba.get_unchecked_mut(i * 4 + 3) = alpha; }
    }
}

fn main() {
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
    glfw.window_hint(glfw::WindowHint::ContextVersion(3, 2));
    glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));
    glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
    glfw.window_hint(glfw::WindowHint::OpenGlDebugContext(true));
    glfw.window_hint(glfw::WindowHint::Resizable(false));

    let cam = touptek::Toupcam::open(None);
    cam.set_preview_size_index(0); // largest

    let (mut window, events) =
        glfw.create_window(1024, 768, "~psychic seamstress~", glfw::WindowMode::Windowed)
            .expect("Failed to create GLFW window.");
    window.set_key_polling(true);
    window.set_sticky_keys(false);
    window.make_current();

    // use glfw to load GL function pointers
    gl!(load_with(|name| window.get_proc_address(name)));
    init_gl();

    cam.start(|cam_events| {
        let nvg = NvgContext::create_gl3(nanovg::ANTIALIAS | nanovg::STENCIL_STROKES);
        let mut ui = ui::UI::new(&nvg);

        while !window.should_close() {
            // Pull new image, if available
            match cam_events.recv().unwrap() {
                touptek::Event::Image => {
                    let mut image = cam.pull_image(32);
                    set_alpha(&mut image.data, 255);
                    ui.cam_image.from_touptek(image)
                },
                touptek::Event::Disconnected => {
                    return
                }
                _ => ()
            }

            let (win_width, win_height) = window.get_size();
            let (fb_width, fb_height) = window.get_framebuffer_size();

            // Update and render
            gl!(Viewport(0, 0, fb_width, fb_height));
            gl!(ClearColor(0.0, 0.0, 0.0, 0.0));
            gl!(Clear(COLOR_BUFFER_BIT | DEPTH_BUFFER_BIT | STENCIL_BUFFER_BIT));

            nvg.begin_frame(win_width, win_height, fb_width as f32 / win_width as f32);
            ui.draw(fb_width as f32, fb_height as f32);
            nvg.end_frame();

            window.swap_buffers();

            glfw.poll_events();
            for (_, event) in glfw::flush_messages(&events) {
                // use glfw::*;
                match event {
                    // WindowEvent::Key(Key::Q, _, Action::Press, _) => {
                    // }
                    _ => {}
                }
            }
        }
    });
}
