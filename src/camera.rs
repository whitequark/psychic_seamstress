extern crate simd;
extern crate touptek;

use std::rc::Rc;
use std::sync::mpsc::{channel, Sender, Receiver, Select};
use std::thread;

use property::Property;

pub enum Event {
    Hotplug(Vec<touptek::Instance>),
    Connect,
    Image(touptek::Image),
    StillImage(touptek::Image),
    Disconnect,
}

enum Command {
    Connect(Option<String>),
    SetExposureTime { microseconds: u32 },
    SetExposureGain { percents: u16 },
    SetColorTemperature { kelvin: u32 },
    SetTint(u32),
    Snap,
}

pub struct Camera {
    cmd_tx: Sender<Command>,
    exposure_time_us: Rc<Property<u32>>,
    exposure_gain_pct: Rc<Property<u16>>,
    color_temperature_k: Rc<Property<u32>>,
    tint: Rc<Property<u32>>,
}

impl Camera {
    pub fn new() -> (Camera, Receiver<Event>) {
        let (event_tx, event_rx) = channel();
        let (cmd_tx, cmd_rx) = channel();

        let exposure_time_us = Property::new(120000);
        exposure_time_us.notify(&cmd_tx, |value|
            Command::SetExposureTime { microseconds: *value });

        let exposure_gain_pct = Property::new(100);
        exposure_gain_pct.notify(&cmd_tx, |value|
            Command::SetExposureGain { percents: *value });

        let color_temperature_k = Property::new(6503);
        color_temperature_k.notify(&cmd_tx, |value|
            Command::SetColorTemperature { kelvin: *value });

        let tint = Property::new(1000);
        tint.notify(&cmd_tx, |value|
            Command::SetTint(*value));

        thread::spawn(move || camera_thread(event_tx, cmd_rx));

        let camera = Camera {
            cmd_tx: cmd_tx,
            exposure_time_us: exposure_time_us,
            exposure_gain_pct: exposure_gain_pct,
            color_temperature_k: color_temperature_k,
            tint: tint,
        };
        (camera, event_rx)
    }

    pub fn connect(&self, unique_id: Option<String>) {
        self.cmd_tx.send(Command::Connect(unique_id)).unwrap();
        self.cmd_tx.send(Command::SetExposureTime {
            microseconds: self.exposure_time_us.get() }).unwrap();
        self.cmd_tx.send(Command::SetExposureGain {
            percents: self.exposure_gain_pct.get() }).unwrap();
        self.cmd_tx.send(Command::SetColorTemperature {
            kelvin: self.color_temperature_k.get() }).unwrap();
        self.cmd_tx.send(Command::SetTint(
            self.tint.get())).unwrap();
    }

    pub fn exposure_time_us(&self) -> Rc<Property<u32>> {
        self.exposure_time_us.clone()
    }

    pub fn exposure_gain_pct(&self) -> Rc<Property<u16>> {
        self.exposure_gain_pct.clone()
    }

    pub fn color_temperature_k(&self) -> Rc<Property<u32>> {
        self.color_temperature_k.clone()
    }

    pub fn tint(&self) -> Rc<Property<u32>> {
        self.tint.clone()
    }

    pub fn snap(&self) {
        self.cmd_tx.send(Command::Snap).unwrap()
    }
}

fn camera_thread(event_tx: Sender<Event>, cmd_rx: Receiver<Command>) {
    event_tx.send(Event::Hotplug(touptek::Toupcam::enumerate())).unwrap();

    touptek::Toupcam::hotplug(|hotplug_rx| {
        loop {
            {
                let select = Select::new();
                let mut cmd_rx = select.handle(&cmd_rx);
                let mut hotplug_rx = select.handle(&hotplug_rx);

                unsafe {
                    cmd_rx.add();
                    hotplug_rx.add();
                }

                loop {
                    let id = select.wait();

                    if id == hotplug_rx.id() {
                        hotplug_rx.recv().unwrap();
                        event_tx.send(Event::Hotplug(touptek::Toupcam::enumerate())).unwrap()
                    }

                    if id == cmd_rx.id() {
                        break
                    }
                }
            }

            let cam =
                match cmd_rx.recv().unwrap() {
                    Command::Connect(camera_id) => {
                        match touptek::Toupcam::open(camera_id.as_ref().map(|s| &s[..])) {
                            Some(camera) => camera,
                            None => continue,
                        }
                    }
                    _ => continue
                };

            cam.set_preview_size_index(0); // largest
            cam.set_automatic_exposure(false);

            cam.start(|cam_rx| {
                event_tx.send(Event::Connect).unwrap();

                let select = Select::new();
                let mut cmd_rx = select.handle(&cmd_rx);
                let mut cam_rx = select.handle(&cam_rx);
                let mut hotplug_rx = select.handle(&hotplug_rx);

                unsafe {
                    cmd_rx.add();
                    cam_rx.add();
                    hotplug_rx.add();
                }

                loop {
                    let id = select.wait();

                    if id == cmd_rx.id() {
                        match cmd_rx.recv().unwrap() {
                            Command::Connect(_) => (),
                            Command::SetExposureTime { microseconds } =>
                                cam.set_exposure_time(microseconds),
                            Command::SetExposureGain { percents } =>
                                cam.set_exposure_gain(percents),
                            Command::SetColorTemperature { kelvin } =>
                                cam.set_white_balance_temp_tint(
                                    touptek::WhiteBalanceTempTint {
                                        temperature: kelvin, ..cam.white_balance_temp_tint() }),
                            Command::SetTint(tint) =>
                                cam.set_white_balance_temp_tint(
                                    touptek::WhiteBalanceTempTint {
                                        tint: tint, ..cam.white_balance_temp_tint() }),
                            Command::Snap =>
                                cam.snap_index(cam.preview_size_index()),
                        }
                    }

                    if id == cam_rx.id() {
                        match cam_rx.recv().unwrap() {
                            touptek::Event::Image => {
                                let mut image = cam.pull_image(32);
                                set_alpha(&mut image.data, 255);
                                event_tx.send(Event::Image(image)).unwrap()
                            },
                            touptek::Event::StillImage => {
                                let mut image = cam.pull_still_image(32);
                                set_alpha(&mut image.data, 255);
                                event_tx.send(Event::StillImage(image)).unwrap()
                            },
                            touptek::Event::Disconnected => {
                                event_tx.send(Event::Disconnect).unwrap();
                                break
                            },
                            touptek::Event::Exposure => {
                                /* ignore */
                            },
                            event => {
                                panic!("unknown camera event: {:?}", event);
                            }
                        }
                    }

                    if id == hotplug_rx.id() {
                        hotplug_rx.recv().unwrap();
                        event_tx.send(Event::Hotplug(touptek::Toupcam::enumerate())).unwrap()
                    }
                }
            })
        }
    })
}

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
