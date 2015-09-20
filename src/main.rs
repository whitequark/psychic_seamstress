#![feature(const_fn, iter_arith, alloc)]
#![allow(unused_unsafe)]

extern crate glfw;
extern crate gl;
extern crate nanovg;
extern crate touptek;
extern crate png;
extern crate simd;

use std::rc::Rc;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use std::path::Path;

use glfw::Context as GlfwContext;
use nanovg::Context as NvgContext;

use ui::*;

mod ui;

enum Event {
    CameraHotplug,
    CameraConnected {
        exposure_time_microseconds: u32,
        exposure_gain_percents: u16,
        color_temperature: u32,
        tint: u32,
    },
    CameraImage(touptek::Image),
    CameraStillImage(touptek::Image),
    CameraDisconnected,
    Glfw(glfw::WindowEvent),
}

enum CameraCmd {
    Connect,
    ExposureTime { microseconds: u32 },
    ExposureGain { percents: u16 },
    ColorTemperature(u32),
    Tint(u32),
    Snap,
}

fn cam_hotplug_thread(tx: Sender<Event>) {
    touptek::Toupcam::hotplug(|rx| {
        loop {
            rx.recv().unwrap();
            tx.send(Event::CameraHotplug).unwrap()
        }
    })
}

fn cam_thread(event_tx: Sender<Event>, cmd_rx: Receiver<CameraCmd>) {
    fn set_alpha(rgba: &mut Vec<u8>, alpha: u8) {
        let alpha = simd::u8x16::new(0, 0, 0, alpha, 0, 0, 0, alpha,
                                     0, 0, 0, alpha, 0, 0, 0, alpha);
        let mut index = 0;
        let length = rgba.len();
        while index < length {
            (simd::u8x16::load( rgba, index) | alpha).store(rgba, index);
            index += 16
        }
    }

    loop {
        match cmd_rx.recv().unwrap() {
            CameraCmd::Connect => (),
            _ => continue
        }

        let cam = match touptek::Toupcam::open(None) {
            Some(cam) => cam,
            None => continue,
        };

        cam.set_preview_size_index(0); // largest
        cam.set_automatic_exposure(false);

        cam.start(|cam_rx| {
            event_tx.send(Event::CameraConnected {
                exposure_time_microseconds: cam.exposure_time(),
                exposure_gain_percents: cam.exposure_gain(),
                color_temperature: cam.white_balance_temp_tint().temperature,
                tint: cam.white_balance_temp_tint().tint,
            }).unwrap();

            loop {
                match cam_rx.recv().unwrap() {
                    touptek::Event::Image => {
                        let mut image = cam.pull_image(32);
                        set_alpha(&mut image.data, 255);
                        event_tx.send(Event::CameraImage(image)).unwrap()
                    },
                    touptek::Event::StillImage => {
                        let mut image = cam.pull_still_image(32);
                        set_alpha(&mut image.data, 255);
                        event_tx.send(Event::CameraStillImage(image)).unwrap()
                    },
                    touptek::Event::Disconnected => {
                        event_tx.send(Event::CameraDisconnected).unwrap();
                        break
                    },
                    touptek::Event::Exposure => {
                        /* ignore */
                    },
                    event => {
                        println!("unknown camera event: {:?}", event);
                    }
                }

                for cmd in glfw::flush_messages(&cmd_rx) {
                    match cmd {
                        CameraCmd::Connect => (),
                        CameraCmd::ExposureTime { microseconds } =>
                            cam.set_exposure_time(microseconds),
                        CameraCmd::ExposureGain { percents } =>
                            cam.set_exposure_gain(percents),
                        CameraCmd::ColorTemperature(temp) =>
                            cam.set_white_balance_temp_tint(
                                touptek::WhiteBalanceTempTint {
                                    temperature: temp, ..cam.white_balance_temp_tint() }),
                        CameraCmd::Tint(tint) =>
                            cam.set_white_balance_temp_tint(
                                touptek::WhiteBalanceTempTint {
                                    tint: tint, ..cam.white_balance_temp_tint() }),
                        CameraCmd::Snap =>
                            cam.snap_index(cam.preview_size_index()),
                    }
                }
            }
        })
    }
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
    let (event_tx, event_rx) = channel();
    event_tx.send(Event::CameraHotplug).unwrap();

    let (cmd_tx, cmd_rx) = channel();
    { let event_tx = event_tx.clone(); thread::spawn(move || { cam_hotplug_thread(event_tx) }) };
    { let event_tx = event_tx.clone(); thread::spawn(move || { cam_thread(event_tx, cmd_rx) }) };

    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
    glfw.window_hint(glfw::WindowHint::ContextVersion(3, 2));
    glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));
    glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
    glfw.window_hint(glfw::WindowHint::OpenGlDebugContext(true));
    // glfw.window_hint(glfw::WindowHint::Resizable(false));

    let (mut window, events) =
        glfw.create_window(1024, 768, "~psychic seamstress~", glfw::WindowMode::Windowed)
            .expect("Failed to create GLFW window.");
    window.set_mouse_button_polling(true);
    window.set_key_polling(true);
    window.set_cursor_pos_polling(true);
    window.set_scroll_polling(true);
    window.make_current();
    { let event_tx = event_tx.clone();
      thread::spawn(move || { glfw_event_thread(events, event_tx) }) };

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

    fn cam_slider<'a, F: Fn(f32) -> CameraCmd + 'static>(
            nvg: &'a NvgContext, cmd_tx: &Sender<CameraCmd>,
            name: String, unit: String, position: SliderPosition,
            map_event: F) -> (BoxLayout<'a>, Rc<Property<SliderPosition>>) {
        let label = Label::new(&nvg);
        let slider = Slider::new(&nvg, position);

        let label_text = label.text();
        let slider_pos = slider.position();

        let cmd_tx = cmd_tx.clone();
        slider_pos.observe(move |position| {
            label_text.set(format!("{}: {}{}", name, position.current, unit));
            cmd_tx.send(map_event(position.current)).unwrap()
        });

        let mut layout = BoxLayout::vert(&nvg);
        layout.add(Box::new(label));
        layout.add(Box::new(slider));

        (layout, slider_pos)
    };

    let mut cfg_layout = BoxLayout::vert(&nvg);

    fn exposure_time_cmd(value: f32) -> CameraCmd {
        CameraCmd::ExposureTime { microseconds: (value * 1000.) as u32 } }
    let (widget, exposure_time_pos) = cam_slider(&nvg, &cmd_tx,
        String::from("Exposure time"), String::from("ms"),
        SliderPosition { minimum: 1., maximum: 2000., step: 5., current: 120. },
        exposure_time_cmd);
    cfg_layout.add(Box::new(widget));

    fn exposure_gain_cmd(value: f32) -> CameraCmd {
        CameraCmd::ExposureGain { percents: value as u16 } }
    let (widget, exposure_gain_pos) = cam_slider(&nvg, &cmd_tx,
        String::from("Exposure gain"), String::from("%"),
        SliderPosition { minimum: 100., maximum: 500., step: 1., current: 100. },
        exposure_gain_cmd);
    cfg_layout.add(Box::new(widget));

    fn color_temp_cmd(value: f32) -> CameraCmd { CameraCmd::ColorTemperature(value as u32) }
    let (widget, color_temp_pos) = cam_slider(&nvg, &cmd_tx,
        String::from("Color temperature"), String::from("K"),
        SliderPosition { minimum: 2000., maximum: 15000., step: 10., current: 6503. },
        color_temp_cmd);
    cfg_layout.add(Box::new(widget));

    fn tint_cmd(value: f32) -> CameraCmd { CameraCmd::Tint(value as u32) }
    let (widget, tint_pos) = cam_slider(&nvg, &cmd_tx,
        String::from("Tint"), String::from(""),
        SliderPosition { minimum: 200., maximum: 2500., step: 10., current: 1000. },
        tint_cmd);
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
                Event::CameraHotplug => {
                    if !camera_connected { cmd_tx.send(CameraCmd::Connect).unwrap() }
                }
                Event::CameraConnected {
                    exposure_time_microseconds, exposure_gain_percents,
                    color_temperature, tint
                } => {
                    camera_connected = true;
                    exposure_time_pos.set(SliderPosition {
                        current: (exposure_time_microseconds / 1000) as f32,
                         ..exposure_time_pos.get()
                    });
                    exposure_gain_pos.set(SliderPosition {
                        current: exposure_gain_percents as f32,
                         ..exposure_gain_pos.get()
                    });
                    color_temp_pos.set(SliderPosition {
                        current: color_temperature as f32,
                         ..color_temp_pos.get()
                    });
                    tint_pos.set(SliderPosition {
                        current: tint as f32,
                         ..tint_pos.get()
                    });
                }
                Event::CameraImage(image) => {
                    ui.background.from_touptek(image);
                },
                Event::CameraStillImage(touptek::Image {
                    resolution: touptek::Resolution { width, height }, data, ..
                }) => {
                    let mut image = png::Image {
                        width: width, height: height,
                        pixels: png::PixelsByColorType::RGBA8(data)
                    };
                    png::store_png(&mut image, Path::new("/tmp/foo.png"));
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
                            ui.mouse_move(Point(x as f32, y as f32) * pixel_ratio),
                        WindowEvent::MouseButton(_button, Action::Press, _modifiers) =>
                            ui.mouse_down(),
                        WindowEvent::MouseButton(_button, Action::Release, _modifiers) =>
                            ui.mouse_up(),
                        WindowEvent::Scroll(x, y) =>
                            ui.mouse_scroll(Point(x as f32, y as f32)),
                        WindowEvent::Key(Key::Space, _, Action::Press, _modifiers) =>
                            cmd_tx.send(CameraCmd::Snap).unwrap(),
                        _ => {}
                    }
                }
            }
        }

        // Poke GLFW
        glfw.poll_events();
    }
}
