use serde_json;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::env;

/// This structure represents a web server, with it's IP address and it's port
#[derive(Debug, Deserialize, Clone)]
pub struct Server {
  pub ip: String,
  pub port: u16,
}

/// This structure represents a load balancer's configuration
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
  pub ip: String,
  pub port: u16,
  pub active_health_check_interval: u64, // seconds
  pub active_health_check_path: String,
  pub rate_limit_window_size: u64, // seconds
  pub max_requests_per_window: u64,
  pub servers: Vec<Server>, // servers list
}

impl Config {
  /// This function assumes that a config file is located at tests/ressources/config.json.
  /// It tries to set up the corresponding config struct.
  pub fn new() -> Config {
    let path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("tests/ressources/config.json");
    let file = File::open(path).expect("Please add a configuration file at tests/ressources/config.json");
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
