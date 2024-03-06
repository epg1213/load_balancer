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
  pub client_map: Arc<Mutex<HashMap<String, u64>>>,
  servers: Arc<Mutex<Vec<bool>>>,
}

impl Balancer {
  pub fn new(config: config::Config) -> Balancer {
    let balancer = Balancer{
      config: config.clone(),
      servers_count: config.servers.len() as u64,
      next: 0,
      client_map: Arc::new(Mutex::new(HashMap::<String, u64>::new())),
      servers: Arc::new(Mutex::new(Vec::<bool>::new())),
    };
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
    let mut client_map = self.client_map.lock().expect("could not read clients");
    match client_map.get_mut(&client_address) {
      Some(count) => {
        *count+=1;
        *count<=self.config.max_requests_per_window
      }
      None => {
        client_map.insert(client_address.clone(), 1);
        true
      }
    }
  }

  pub async fn check_s(&self) -> bool {
    let servers = self.servers.lock().unwrap();
    return servers[self.next];
  }

  pub async fn get_server(&mut self, client_address: String) -> Result<&config::Server, &'static str> {
    if !self.client_rate_ok(client_address){
      return Err("429 Too many Requests.");
    }
    let mut down_servers=0;
    self.next=((self.next+1) as u64 % self.servers_count) as usize;
    while !(self.check_s().await) {
      down_servers+=1;
      if down_servers==self.servers_count {
        return Err("502 Bad Gateway.");
      }
      self.next=((self.next+1) as u64 % self.servers_count) as usize;
    }
    return Ok(&self.config.servers[self.next]);
  }
  
  async fn check_servers(target_servers: Vec<config::Server>, healthpath: String) -> Vec<bool> {
    let mut vec= Vec::<bool>::new();
    for server in target_servers.iter()
    {
      vec.push(Balancer::check(server, healthpath.clone()).await);
    }
    return vec;
  }

  async fn verify_servers(target_servers: Vec<config::Server>, servers_status: Arc<Mutex<Vec<bool>>>, interval: u64, healthpath: String) {
    loop {
      let servers_checked = Balancer::check_servers(target_servers.clone(), healthpath.clone()).await;
      {
        let mut servers = servers_status.lock().expect("could not read servers"); // vérouille un thread pour pas qu'un thread modifie un autre thread
        let _ = mem::replace(&mut *servers, servers_checked);
      }
      sleep(Duration::from_secs(interval));
    }
  }
  async fn clean_clients( client_map: Arc<Mutex<HashMap<String, u64>>>, interval: u64) {
    /*loop {
      {
        let mut map = client_map.lock().expect("could not read clients"); // vérouille un thread pour pas qu'un thread modifie un autre thread
        mem::replace(&mut *map, HashMap::<String, u64>::new(),);
        println!("cleaning clients");
      }
      sleep(Duration::from_secs(interval));
    }*/
  }

  pub fn start_threads(&self) {
    let target_servers = self.config.servers.clone();
    let servers_status = self.servers.clone();
    let interval = self.config.active_health_check_interval.clone();
    let healthpath = self.config.active_health_check_path.clone();
    tokio::spawn(async move { Balancer::verify_servers(target_servers, servers_status, interval, healthpath).await });
    /*let map = self.client_map.clone();
    let window = self.config.rate_limit_window_size.clone();
    tokio::spawn(async move { Balancer::clean_clients(map, window).await });*/
  }
}

