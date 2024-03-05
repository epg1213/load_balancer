use std::collections::HashMap;
extern crate reqwest;
use crate::config;
use std::thread::spawn;
use std::sync::{Arc, Mutex};
use std::thread::scope;
use std::thread::sleep;
use std::mem;
use std::time::Duration;
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
    balancer.verify();
    balancer
  }

  async fn check(&self, server: &config::Server) -> bool {
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

  pub async fn get_server(&mut self, client_address: String) -> Result<&config::Server, &'static str> {
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
  
  fn check_servers(&self) -> Vec<bool> {

    let mut vec= Vec::<bool>::new();
    /*for server in self.config.servers.iter()
    {
      vec.push(self.check(server).await);
      

    }*/
    vec.push(true);
    vec.push(false);
    vec.push(true);
    return vec;
  }

  fn verify(&self) {
    let verify_thread = ||{
      loop {
        let servers_checked = self.check_servers();
        {
          let mut servers = self.servers.lock().expect("could not read servers"); // vérouille un thread pour pas qu'un thread modifie un autre thread
          mem::replace(&mut *servers, servers_checked);
          println!("{:?}", servers);
        }
        sleep(Duration::from_secs(self.config.active_health_check_interval));
      }
    };
    
    std::thread::scope(|scope| {
      scope.spawn(verify_thread);
    });

  }
}

