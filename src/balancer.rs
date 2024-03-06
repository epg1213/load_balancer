use std::collections::HashMap;
extern crate reqwest;
use crate::config;
use std::sync::{Arc, Mutex};
use std::mem;
use std::time::Duration;
use std::thread::sleep;
pub struct Balancer {
  pub config: config::Config,
  pub servers_count: u64,
  pub next: usize,
  pub client_map: HashMap<String, u64>,
  servers: Arc<Mutex<Vec<bool>>>,
}

impl Balancer {
  pub fn new(config: config::Config) -> Balancer {
    let balancer = Balancer{
      config: config.clone(),
      servers_count: config.servers.len() as u64,
      next: 0,
      client_map: HashMap::<String, u64>::new(),
      servers: Arc::new(Mutex::new(Vec::<bool>::new())),
    };
    //balancer.verify();
    balancer
  }

  async fn check(server: &config::Server, healthpath: String) -> bool {
    match reqwest::get(format!("http://{}:{}{}", server.ip, server.port, healthpath)).await {
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

  pub async fn get_server(&mut self, client_address: String) -> Result<&config::Server, &'static str> {
    if !self.client_rate_ok(client_address){
      return Err("429 Too many Requests.");
    }
    let mut down_servers=0;
    self.next=((self.next+1) as u64 % self.servers_count) as usize;
    let mut server=&self.config.servers[self.next];
    /*while !(self.check(server).await) {
      down_servers+=1;
      if down_servers==self.servers_count {
        return Err("502 Bad Gateway.");
      }
      self.next=((self.next+1) as u64 % self.servers_count) as usize;
      server=&self.config.servers[self.next];
    }*/
    return Ok(server);
  }
  
  async fn check_servers(target_servers: Vec<config::Server>, healthpath: String) -> Vec<bool> {
    let mut vec= Vec::<bool>::new();
    for server in target_servers.iter()
    {
      vec.push(Balancer::check(server, healthpath.clone()).await);
    }
    return vec;
  }

  async fn verify(target_servers: Vec<config::Server>, servers_status: Arc<Mutex<Vec<bool>>>, interval: u64, healthpath: String) {
    loop {
      let servers_checked = Balancer::check_servers(target_servers.clone(), healthpath.clone()).await;
      {
        let mut servers = servers_status.lock().expect("could not read servers"); // vérouille un thread pour pas qu'un thread modifie un autre thread
        mem::replace(&mut *servers, servers_checked);
      }
      sleep(Duration::from_secs(interval));
    }
  }

  pub fn start_thread(&self) {
    let target_servers = self.config.servers.clone();
    let servers_status = self.servers.clone();
    let interval = self.config.active_health_check_interval.clone();
    let healthpath = self.config.active_health_check_path.clone();
    tokio::spawn(async move { Balancer::verify(target_servers, servers_status, interval, healthpath).await });
  }
}

