#![feature(const_fn, iter_arith)]
#![allow(unused_unsafe)]

extern crate glfw;
extern crate gl;
extern crate nanovg;
extern crate touptek;
extern crate png;

use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

use glfw::Context as GlfwContext;
use nanovg::Context as NvgContext;

use ui::widget::Container;

mod ui;

enum Event {
    CameraHotplug,
    CameraConnected,
    CameraImage(touptek::Image),
    CameraDisconnected,
    Glfw(glfw::WindowEvent),
}

fn cam_hotplug_thread(tx: Sender<Event>) {
    touptek::Toupcam::hotplug(|rx| {
        loop {
            rx.recv().unwrap();
            tx.send(Event::CameraHotplug).unwrap()
        }
    })
}

fn cam_thread(tx: Sender<Event>) {
    fn set_alpha(rgba: &mut Vec<u8>, alpha: u8) {
        for i in 0..rgba.len() / 4 {
            unsafe { *rgba.get_unchecked_mut(i * 4 + 3) = alpha; }
        }
    }

    let cam = touptek::Toupcam::open(None);
    cam.set_preview_size_index(0); // largest

    cam.start(|rx| {
        tx.send(Event::CameraConnected).unwrap();

        loop {
            match rx.recv().unwrap() {
                touptek::Event::Image => {
                    let mut image = cam.pull_image(32);
                    set_alpha(&mut image.data, 255);
                    tx.send(Event::CameraImage(image)).unwrap()
                },
                touptek::Event::Disconnected => {
                    tx.send(Event::CameraDisconnected).unwrap();
                    break
                }
                _ => ()
            }
        }
    })
}

fn glfw_event_thread(rx: Receiver<(f64, glfw::WindowEvent)>, tx: Sender<Event>) {
    loop {
        let (_time, event) = rx.recv().unwrap();
        tx.send(Event::Glfw(event)).unwrap();
    }
}

macro_rules! gl {
    ($e: expr) => ({
        use gl::*;
        unsafe { $e };
        assert_eq!(unsafe { GetError() }, 0);
    })
}

fn main() {
    let (tx, rx) = channel();

    { let tx = tx.clone(); thread::spawn(move || { cam_hotplug_thread(tx) }) };
    { let tx = tx.clone(); thread::spawn(move || { cam_thread(tx) }) };

    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
    glfw.window_hint(glfw::WindowHint::ContextVersion(3, 2));
    glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));
    glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
    glfw.window_hint(glfw::WindowHint::OpenGlDebugContext(true));
    glfw.window_hint(glfw::WindowHint::Resizable(false));

    let (mut window, events) =
        glfw.create_window(1024, 768, "~psychic seamstress~", glfw::WindowMode::Windowed)
            .expect("Failed to create GLFW window.");
    window.set_mouse_button_polling(true);
    window.set_cursor_pos_polling(true);
    window.make_current();
    { let tx = tx.clone(); thread::spawn(move || { glfw_event_thread(events, tx) }) };

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

    let mut cfg_layout = ui::BoxLayout::vert(&nvg);
    cfg_layout.add(Box::new(ui::Label::new(&nvg, "test")));
    cfg_layout.add(Box::new(ui::Label::new(&nvg, "foobar")));
    cfg_layout.add(Box::new(ui::Label::new(&nvg, "baz")));

    let mut cfg_frame = ui::Frame::new(&nvg, Box::new(cfg_layout));
    cfg_frame.set_position(ui::Point(20.0, 20.0));

    let mut ui = ui::Overlay::new(&nvg);
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

        nvg.begin_frame(win_width, win_height, pixel_ratio);
        ui.draw(ui::Point(fb_width as f32, fb_height as f32));
        nvg.end_frame();

        window.swap_buffers();

        // Handle events
        for event in glfw::flush_messages(&rx) {
            match event {
                Event::CameraHotplug => {
                    if !camera_connected {
                        let tx = tx.clone(); thread::spawn(move || { cam_thread(tx) });
                    }
                }
                Event::CameraConnected => {
                    camera_connected = true;
                }
                Event::CameraImage(image) => {
                    ui.background.from_touptek(image);
                },
                Event::CameraDisconnected => {
                    camera_connected = false;
                    ui.background.from_png(png::load_png("res/nosignal.png").unwrap())
                }
                Event::Glfw(event) => {
                    use glfw::*;
                    // println!("{:?}", event);
                    match event {
                        WindowEvent::CursorPos(x, y) =>
                            ui.mouse_move(ui::Point(x as f32 * pixel_ratio,
                                                    y as f32 * pixel_ratio)),
                        WindowEvent::MouseButton(_button, Action::Press, _) =>
                            ui.mouse_down(),
                        WindowEvent::MouseButton(_button, Action::Release, _) =>
                            ui.mouse_up(),
                        _ => {}
                    }
                }
            }
        }

        // Poke GLFW
        glfw.poll_events();
    }
}
