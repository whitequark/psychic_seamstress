extern crate xdg;
extern crate serde_json;

use std::rc::Rc;
use std::fs::File;

use property::Property;

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    exposure_time_us: Rc<Property<u32>>,
    exposure_gain_pct: Rc<Property<u16>>,
    color_temperature_k: Rc<Property<u32>>,
    tint: Rc<Property<u32>>,
}

impl Config {
    pub fn exposure_time_us(&self) -> Rc<Property<u32>> { self.exposure_time_us.clone() }
    pub fn exposure_gain_pct(&self) -> Rc<Property<u16>> { self.exposure_gain_pct.clone() }
    pub fn color_temperature_k(&self) -> Rc<Property<u32>> { self.color_temperature_k.clone() }
    pub fn tint(&self) -> Rc<Property<u32>> { self.tint.clone() }
}

fn xdg_dirs() -> xdg::BaseDirectories {
    xdg::BaseDirectories::with_prefix("psychic_seamstress")
}

pub fn load() -> Config {
    match xdg_dirs().find_config_file("config.json") {
        None => Config::default(),
        Some(path) => {
            let mut file = File::open(path).unwrap();
            serde_json::from_reader(&mut file).unwrap()
        }
    }
}

pub fn store(config: &Config) {
    let path = xdg_dirs().place_config_file("config.json").unwrap();
    let mut file = File::create(path).unwrap();
    serde_json::to_writer_pretty(&mut file, config).unwrap()
}
