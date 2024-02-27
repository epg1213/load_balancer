use actix_web::{get, web, HttpResponse, HttpServer, App};
use actix_proxy::{IntoHttpResponse, SendRequestError};
use awc::Client;
use std::sync::Mutex;
mod balancer;

#[get("/{url:.*}")]
async fn proxy( app_balancer: web::Data<Mutex<balancer::Balancer>>,
  path: web::Path<(String,)>,
) -> Result<HttpResponse, SendRequestError> {
  let mut load_balancer = app_balancer.lock().unwrap();
  match load_balancer.get_server().await {
    Ok(srv) => {
      let (url,) = path.into_inner();
      let url = format!("http://{}:{}/{}", srv.ip, srv.port, url);
      let client = Client::new();
      Ok(client.get(&url).send().await?.into_http_response())
    }
    Err(err) => {
      Ok(HttpResponse::Ok().body(err))
    }
  }

}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  let config = balancer::Config::new();
  let load_balancer = web::Data::new(Mutex::new(balancer::Balancer::new(config.clone())));
  HttpServer::new(move|| {
    App::new()
      .app_data(load_balancer.clone())
      .service(proxy)
  })
  .bind((config.ip, config.port))?
  .run()
  .await
}
