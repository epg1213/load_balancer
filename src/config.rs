use serde_json;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;

#[derive(Debug, Deserialize, Clone)]
pub struct Server {
  pub ip: String,
  pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
  pub ip: String,
  pub port: u16,
  pub active_health_check_interval: u64,
  pub active_health_check_path: String,
  pub rate_limit_window_size: u64,
  pub max_requests_per_window: u64,
  pub servers: Vec<Server>,
}

impl Config {
  pub fn new() -> Config {
    let file = File::open("config.json").expect("Could not read config file, please add a config.json file.");
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).expect("Could not parse config file.")
  }
}
