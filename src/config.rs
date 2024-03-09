use serde_json;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::env;

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
    let path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("tests/ressources/config.json");
    let file = File::open(path).expect("Could not open config file");
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).expect("Could not parse config file.")
  }
}

#[cfg(test)]
mod test{
  use std::path::Path;
  use std::env;
  #[test]
  fn test_new(){
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let config_path = Path::new(&manifest_dir).join("tests/ressources/config.json");
    assert!(config_path.exists(), "le fichier dois exister"); 
  }
  
}
