use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use serde_json;
extern crate reqwest;

#[derive(Debug, Deserialize, Clone)]
pub struct Server {
  pub ip: String,
  pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
  pub ip: String,
  pub port: u16,
  pub servers: Vec<Server>,
}

impl Config {
  pub fn new() -> Config {
    let file = File::open("config.json").expect("Could not read config file, please add a config.json file.");
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).expect("Could not parse config file.")
  }
}

pub struct Balancer {
  pub config: Config,
  pub servers_count: u64,
  pub next: usize,
}

impl Balancer {
  pub fn new(config: Config) -> Balancer {
    Balancer{
      config: config.clone(),
      servers_count: config.servers.len() as u64,
      next: 0,
    }
  }

  async fn check(server: &Server) -> bool {
    match reqwest::get(format!("http://{}:{}/healthcheck", server.ip, server.port)).await {
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

  pub async fn get_server(&mut self) -> Result<&Server, &'static str> {
    let mut down_servers=0;
    self.next=((self.next+1) as u64 % self.servers_count) as usize;
    let mut server=&self.config.servers[self.next];
    while !(Balancer::check(server).await) {
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
