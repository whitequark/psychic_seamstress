#![feature(const_fn, iter_arith, plugin, custom_derive, mpsc_select, drain)]
#![allow(unused_unsafe, dead_code)]
// #![plugin(serde_macros)]

extern crate glfw;
extern crate gl;
extern crate nanovg;
extern crate png;
// extern crate serde;
extern crate touptek;

use std::rc::Rc;
use std::cell::RefCell;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::path::Path;
use std::thread;

use glfw::Context as GlfwContext;
use nanovg::Context as NvgContext;

use property::Property;
use ui::*;

pub mod property;
// pub mod config;
pub mod camera;
pub mod ui;

macro_rules! gl {
    ($e: expr) => ({
        use gl::*;
        unsafe { $e };
        assert_eq!(unsafe { GetError() }, 0);
    })
}

fn main() {
    // let config = Rc::new(RefCell::new(config::load()));

    enum Event {
        Camera(camera::Event),
        Glfw(glfw::WindowEvent),
    }
    let (event_tx, event_rx) = channel();

    let (mut camera, camera_event_rx) = camera::Camera::new();
    {
        let event_tx = event_tx.clone();
        thread::spawn(move || {
            loop {
                let event = camera_event_rx.recv().unwrap();
                event_tx.send(Event::Camera(event)).unwrap();
            }
        });
    }

    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
    glfw.window_hint(glfw::WindowHint::ContextVersion(3, 2));
    glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));
    glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
    glfw.window_hint(glfw::WindowHint::OpenGlDebugContext(true));
    // glfw.window_hint(glfw::WindowHint::Resizable(false));

    let (mut window, glfw_event_rx) =
        glfw.create_window(1024, 768, "~psychic seamstress~", glfw::WindowMode::Windowed)
            .expect("Failed to create GLFW window.");
    window.set_mouse_button_polling(true);
    window.set_key_polling(true);
    window.set_cursor_pos_polling(true);
    window.set_scroll_polling(true);
    window.make_current();
    {
        let event_tx = event_tx.clone();
        thread::spawn(move || {
            loop {
                let (_time, event) = glfw_event_rx.recv().unwrap();
                event_tx.send(Event::Glfw(event)).unwrap()
            }
        })
    };

    // use glfw to load GL function pointers
    gl!(load_with(|name| window.get_proc_address(name)));
    gl!(FrontFace(CCW));
    gl!(CullFace(BACK));
    gl!(Enable(CULL_FACE));
    gl!(Enable(BLEND));
    gl!(BlendFunc(SRC_ALPHA, ONE_MINUS_SRC_ALPHA));
    gl!(Enable(SCISSOR_TEST));

    let nvg = NvgContext::create_gl3(nanovg::ANTIALIAS | nanovg::STENCIL_STROKES);
    nvg.create_font("Roboto", "res/Roboto-Regular.ttf").unwrap();

    let mut cfg_layout = BoxLayout::vert(&nvg);

    fn slider<'a>(nvg: &'a NvgContext, name: String, unit: String, position: SliderPosition) ->
                        (BoxLayout<'a>, Rc<Property<SliderPosition>>) {
        let label = Label::new(&nvg);
        let slider = Slider::new(&nvg, position);

        let slider_pos = slider.position();
        slider_pos.propagate(label.text(), move |position|
            format!("{}: {}{}", name, position.current, unit));

        let mut layout = BoxLayout::vert(&nvg);
        layout.add(Box::new(label));
        layout.add(Box::new(slider));

        (layout, slider_pos)
    }

    // Exposure time slider
    let (widget, exposure_time_pos) = slider(&nvg,
        "Exposure time".to_string(), "ms".to_string(),
        SliderPosition { minimum: 1., maximum: 2000., step: 5., current: 0. });
    camera.exposure_time_us().link(exposure_time_pos.clone(),
       |slider, value| SliderPosition { current: (value / 1000) as f32, ..*slider },
       |slider|        (slider.current * 1000.) as u32);
    cfg_layout.add(Box::new(widget));

    // Exposure gain slider
    let (widget, exposure_gain_pos) = slider(&nvg,
        "Exposure gain".to_string(), "%".to_string(),
        SliderPosition { minimum: 100., maximum: 500., step: 1., current: 0. });
    camera.exposure_gain_pct().link(exposure_gain_pos.clone(),
        |slider, value| SliderPosition { current: value as f32, ..*slider },
        |slider|        slider.current as u16);
    cfg_layout.add(Box::new(widget));

    // Color temperature slider
    let (widget, color_temp_pos) = slider(&nvg,
        "Color temperature".to_string(), "K".to_string(),
        SliderPosition { minimum: 2000., maximum: 15000., step: 10., current: 0. });
    camera.color_temperature_k().link(color_temp_pos.clone(),
        |slider, value| SliderPosition { current: value as f32, ..*slider },
        |slider|        slider.current as u32);
    cfg_layout.add(Box::new(widget));

    // Tint slider
    let (widget, tint_pos) = slider(&nvg,
        "Tint".to_string(), "".to_string(),
        SliderPosition { minimum: 200., maximum: 2500., step: 10., current: 0. });
    camera.tint().link(tint_pos.clone(),
        |slider, value| SliderPosition { current: value as f32, ..*slider },
        |slider|        slider.current as u32);
    cfg_layout.add(Box::new(widget));

    let mut cfg_frame = Frame::new(&nvg, Box::new(cfg_layout));
    cfg_frame.set_position(Point(20.0, 20.0));

    let mut ui = Overlay::new(&nvg);
    ui.background.from_png(png::load_png("res/nosignal.png").unwrap());
    ui.frames.push(cfg_frame);

    let mut camera_connected = false;
    while !window.should_close() {
        // Check if window was resized
        let (win_width, win_height) = window.get_size();
        let (fb_width, fb_height) = window.get_framebuffer_size();
        let pixel_ratio = fb_width as f32 / win_width as f32;

        // Reflow UI
        ui.prepare();

        // Render UI
        gl!(Viewport(0, 0, fb_width, fb_height));
        gl!(ClearColor(0.0, 0.0, 0.0, 0.0));
        gl!(Clear(COLOR_BUFFER_BIT | DEPTH_BUFFER_BIT | STENCIL_BUFFER_BIT));

        nvg.begin_frame(win_width as u32, win_height as u32, pixel_ratio);
        ui.draw(Point(fb_width as f32, fb_height as f32));
        nvg.end_frame();

        window.swap_buffers();

        // Handle events
        for event in glfw::flush_messages(&event_rx) {
            match event {
                Event::Camera(camera::Event::Hotplug(_)) => {
                    if !camera_connected { camera.connect(None) }
                }
                Event::Camera(camera::Event::Connect) => {
                    camera_connected = true;
                }
                Event::Camera(camera::Event::Image(image)) => {
                    ui.background.from_touptek(image);
                }
                Event::Camera(camera::Event::StillImage(touptek::Image {
                    resolution: touptek::Resolution { width, height }, data, ..
                })) => {
                    let mut image = png::Image {
                        width: width, height: height,
                        pixels: png::PixelsByColorType::RGBA8(data)
                    };
                    png::store_png(&mut image, Path::new("/tmp/foo.png")).unwrap()
                }
                Event::Camera(camera::Event::Disconnect) => {
                    camera_connected = false;
                    ui.background.from_png(png::load_png("res/nosignal.png").unwrap())
                }
                Event::Glfw(event) => {
                    use glfw::*;
                    // println!("{:?}", event);
                    match event {
                        WindowEvent::CursorPos(x, y) =>
                            ui.mouse_move(Point(x as f32, y as f32) * pixel_ratio),
                        WindowEvent::MouseButton(_button, Action::Press, _modifiers) =>
                            ui.mouse_down(),
                        WindowEvent::MouseButton(_button, Action::Release, _modifiers) =>
                            ui.mouse_up(),
                        WindowEvent::Scroll(x, y) =>
                            ui.mouse_scroll(Point(x as f32, y as f32)),
                        WindowEvent::Key(Key::Space, _, Action::Press, _modifiers) =>
                            camera.snap(),
                        WindowEvent::Key(Key::Escape, _, Action::Press, _modifiers) => {
                            // config::store(&*config.borrow());
                            return
                        }
                        _ => {}
                    }
                }
            }
        }

        // Poke GLFW
        glfw.poll_events();
    }
}
