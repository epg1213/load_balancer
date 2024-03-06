use actix_web::{get, web, HttpRequest, HttpResponse, HttpServer, App};
use actix_proxy::{IntoHttpResponse, SendRequestError};
use awc::Client;
use std::sync::Mutex;
mod balancer;
mod config;

#[get("/{url:.*}")]
async fn proxy( app_balancer: web::Data<Mutex<balancer::Balancer>>,
path: web::Path<(String,)>,
req: HttpRequest
) -> Result<HttpResponse, SendRequestError> {
let mut client_address = String::new();
if let Some(addr) = req.peer_addr() {
    client_address = format!("{}", addr.ip());
}
let mut load_balancer = app_balancer.lock().expect("could not lock balancer");
match load_balancer.get_server(client_address).await {
    Ok(srv) => {
    let (url,) = path.into_inner();
    let url = format!("http://{}:{}/{}", srv.ip, srv.port, url);
    let client = Client::new();
    Ok(client.get(&url).send().await?.into_http_response())
    }
    Err(err) => {
    if err.contains("429") {
        return Ok(HttpResponse::TooManyRequests().body(err));
    } else if err.contains("502") {
        return Ok(HttpResponse::BadGateway().body(err));
    }
    Ok(HttpResponse::InternalServerError().body(err))
    }
}

}

#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
let config = config::Config::new();
let balancer = balancer::Balancer::new(config.clone());
balancer.start_threads();
let load_balancer = web::Data::new(Mutex::new(balancer));
HttpServer::new(move|| {
    App::new()
    .app_data(load_balancer.clone())
    .service(proxy)
})
.bind((config.ip, config.port))
.expect("Cannot start server on this address.")
.run()
.await
}
