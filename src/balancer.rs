use std::collections::HashMap;
extern crate reqwest;
use crate::config;
use std::sync::{Arc, Mutex};
use std::mem;
use std::time::Duration;
use std::thread::sleep;

/// This structure represents the load balancer itself.
pub struct Balancer {
  pub configuration: config::Config, // the config file loaded in a struct
  pub servers_count: u64,
  pub next: usize, // the next server's index
  pub client_map: Arc<Mutex<HashMap<String, u64>>>, // every client's requests count
  servers: Arc<Mutex<Vec<bool>>>, // to keep track of all server statuses
}

impl Balancer {
  /// This function takes a configuration as a parameter and creates the associated load balancer.
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

  /// This function takes a web server as a parameter and returns a boolean.
  /// It is responsible for checking whether a specific server is up and healthy or not.
  async fn check(server: &config::Server, healthpath: String) -> bool {
    // build and get the server's healthcheck URL
    match reqwest::get(format!("http://{}:{}{}", server.ip, server.port, healthpath)).await {
      Ok(response) => {
        if response.status() == reqwest::StatusCode::OK {
          match response.text().await {
            Ok(_) => return true, // if the server's response is 200, then it is fine
            Err(_) => return false //if an error occurs, consider the server down
          }
        }
        else {
          //a response different from 200 means the server is in bad health
          return false
        }
      }
      Err(_) => return false
    }
  }

  /// This function verifies if a given client (getting it's IP as a string) has generated too much traffic
  /// It returns a boolean indicating if the client can make more requests or not.
  fn client_rate_ok(&mut self, client_address: String) -> bool {
    // securing all clients data in order to access it
    let mut client_map = self.client_map.lock().expect("could not read clients");
    match client_map.get_mut(&client_address) { // getting the associated requests count
      Some(count) => {
        *count+=1; // if the client has made other requests, the counter goes up by one
        *count<=self.configuration.max_requests_per_window // checks if the max has been reached
      }
      None => {
        client_map.insert(client_address.clone(), 1); //if it's a client's first request, set the count to 1
        true // the max has not been reached
      }
    }
  }

  /// This function returns whether the next server in the list is available
  pub async fn check_srv(&self) -> bool {
    let servers = self.servers.lock().unwrap();
    return servers[self.next];
  }

  /// This function tries to return a server to the specified client, using the servers list.
  /// If this client is blacklisted, the return value will be an error containing a 429 code.
  /// If no server is available, the return value will be an error containing a 502 code.
  /// Otherwise, a healthy server
  pub async fn get_server(&mut self, client_address: String) -> Result<&config::Server, &'static str> {
    if !self.client_rate_ok(client_address){
      return Err("429 Too many Requests."); // the client is already (or gets) blacklisted
    }
    let mut down_servers=0;
    self.next=((self.next+1) as u64 % self.servers_count) as usize; // reaches out for the next server
    while !(self.check_srv().await) { // while we didn't find any healthy server
      down_servers+=1;
      if down_servers==self.servers_count { // all servers are down
        return Err("502 Bad Gateway.");
      }
      self.next=((self.next+1) as u64 % self.servers_count) as usize;
    }
    return Ok(&self.configuration.servers[self.next]); // returns a healthy server (hopefully)
  }
  
  /// This function updates the health status for each server
  /// The servers addresses list gets passed as a parameter, as well as the generic healthcheck URL.
  /// It returns a vector of booleans indicating whether each server is up and well or not
  async fn check_servers(target_servers: Vec<config::Server>, healthpath: String) -> Vec<bool> {
    let mut vec= Vec::<bool>::new();
    for server in target_servers.iter()
    {
      vec.push(Balancer::check(server, healthpath.clone()).await); // checks each server
    }
    return vec;
  }

  /// This function gets started in a new thread and is responsible for starting a healthcheck every n seconds
  /// The servers addresses list gets passed as a parameter "target_servers"
  /// The servers statuses to update gets passed as "server_status"
  /// The number of seconds to wait between each healthcheck is "interval"
  /// The healthcheck URL is "healthpath"
  async fn verify_servers(target_servers: Vec<config::Server>, servers_status: Arc<Mutex<Vec<bool>>>, interval: u64, healthpath: String) {
    loop {
      let servers_checked = Balancer::check_servers(target_servers.clone(), healthpath.clone()).await;
      {
        let mut servers = servers_status.lock().expect("could not read servers");
        let _ = mem::replace(&mut *servers, servers_checked);
      }
      sleep(Duration::from_secs(interval));
    }
  }

  /// This function is still in development, as the lock for the client_map cant happen as intended for now
  /// It should clear the clients map in order to give access to clients that were blocked after a certain amount of time
  /// The clients map as well as the time to wait before cleaning it gets passed as parameters
  async fn clean_clients( client_map: Arc<Mutex<HashMap<String, u64>>>, interval: u64) {
    loop {
      {
        let mut map = client_map.lock().expect("could not read clients");
        let _ = mem::replace(&mut *map, HashMap::<String, u64>::new(),);
        println!("cleaning clients");
      }
      sleep(Duration::from_secs(interval));
    }
  }

  /// This function is responsible for starting the two threads "serify_servers" and "clean_clients"
  /// For now, the clean_clients function isn't called as it does not work properly.
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