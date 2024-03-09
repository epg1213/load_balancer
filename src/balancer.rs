use std::collections::HashMap;
extern crate reqwest;
use crate::config;
use std::sync::{Arc, Mutex};
use std::mem;
use std::time::Duration;
use std::thread::sleep;

pub struct Balancer {
  pub configuration: config::Config,
  pub servers_count: u64,
  pub next: usize,
  pub client_map: Arc<Mutex<HashMap<String, u64>>>,
  servers: Arc<Mutex<Vec<bool>>>,
}

impl Balancer {
  pub fn new(config: config::Config) -> Balancer {
    let balancer = Balancer{
      configuration: config.clone(),
      servers_count: config.servers.len() as u64,
      next: 0,
      client_map: Arc::new(Mutex::new(HashMap::<String, u64>::new())),
      servers: Arc::new(Mutex::new(vec![false; config.servers.len()])),
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
        *count<=self.configuration.max_requests_per_window
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
    return Ok(&self.configuration.servers[self.next]);
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
    loop {
      {
        let mut map = client_map.lock().expect("could not read clients"); // vérouille un thread pour pas qu'un thread modifie un autre thread
        let _ = mem::replace(&mut *map, HashMap::<String, u64>::new(),);
        println!("cleaning clients");
      }
      sleep(Duration::from_secs(interval));
    }
  }

  pub fn start_threads(&self) {
    let target_servers = self.configuration.servers.clone();
    let servers_status = self.servers.clone();
    let interval = self.configuration.active_health_check_interval.clone();
    let healthpath = self.configuration.active_health_check_path.clone();
    tokio::spawn(async move { Balancer::verify_servers(target_servers, servers_status, interval, healthpath).await });
    /*let map = self.client_map.clone();
    let window = self.config.rate_limit_window_size.clone();
    tokio::spawn(async move { Balancer::clean_clients(map, window).await });*/
  }
}


#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;
  use serde_json;
  use crate::config;
  use mockito::Matcher;
  use tokio;

  #[test]
    fn test_new_balancer() {
    let config = config::Config::new();

    let balancer = Balancer::new(config.clone());
    assert_eq!(balancer.servers_count, config.servers.len() as u64);
    assert_eq!(balancer.next, 0);
    assert_eq!(*balancer.client_map.lock().unwrap(), HashMap::new());
    assert_eq!(*balancer.servers.lock().unwrap(), vec![false; config.servers.len()]);
  }
  
  #[tokio::test]
  async fn test_check() {
    let mut s = mockito::Server::new();
    let m = s.mock("GET", "/healthcheck")
      .with_status(200)
      .create();
    let mut server_url = s.url();
    server_url = String::from(server_url.trim_start_matches("http://"));
    let parts: Vec<&str> = server_url.split(':').collect();
    let server = config::Server {
        ip: parts[0].to_string(),
        port: parts[1].parse().unwrap(),
    };
    let healthpath = String::from("/healthcheck");

    // Testez que la fonction check renvoie true lorsque le serveur renvoie 200
    let result = Balancer::check(&server, healthpath).await;
    assert!(result);
}
}