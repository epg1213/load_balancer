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
  _active_health_check_interval: u64,
  active_health_check_path: String,
  _rate_limit_window_size: u64,
  max_requests_per_window: u64,
  pub servers: Vec<Server>,
}

impl Config {
  pub fn new() -> Config {
    let file = File::open("config.json").expect("Could not read config file, please add a config.json file.");
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).expect("Could not parse config file.")
  }
}
use std::collections::HashMap;
extern crate reqwest;


pub struct Balancer {
  pub config: Config,
  pub servers_count: u64,
  pub next: usize,
  pub client_map: HashMap<String, u64>,
}

impl Balancer {
  pub fn new(config: Config) -> Balancer {
    Balancer{
      config: config.clone(),
      servers_count: config.servers.len() as u64,
      next: 0,
      client_map: HashMap::<String, u64>::new(),
    }
  }

  async fn check(&self, server: &Server) -> bool {
    match reqwest::get(format!("http://{}:{}{}", server.ip, server.port, self.config.active_health_check_path)).await {
      Ok(response) => {
        if response.status() == reqwest::StatusCode::OK {
          match response.text().await {
            Ok(_) => return true,
            Err(_) => return false
          }
        }
        else {
          //Response not 200
          return false
        }
      }
      Err(_) => return false
    }
  }

  fn client_rate_ok(&mut self, client_address: String) -> bool {
    match self.client_map.get_mut(&client_address) {
      Some(count) => {
        *count+=1;
        *count<=self.config.max_requests_per_window
      }
      None => {
        self.client_map.insert(client_address.clone(), 1);
        true
      }
    }
  }

  pub async fn get_server(&mut self, client_address: String) -> Result<&Server, &'static str> {
    if !self.client_rate_ok(client_address){
      return Err("429 Too many Requests.");
    }
    let mut down_servers=0;
    self.next=((self.next+1) as u64 % self.servers_count) as usize;
    let mut server=&self.config.servers[self.next];
    while !(self.check(server).await) {
      down_servers+=1;
      if down_servers==self.servers_count {
        return Err("502 Bad Gateway.");
      }
      self.next=((self.next+1) as u64 % self.servers_count) as usize;
      server=&self.config.servers[self.next];
    }
    return Ok(server);
  }
}
