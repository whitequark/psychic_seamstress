extern crate xdg;
extern crate serde_json;

use std::fs::File;

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Config {
    pub exposure_time_milliseconds: u32,
    pub exposure_gain_percents: u16,
    pub color_temperature: u32,
    pub tint: u32,
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
