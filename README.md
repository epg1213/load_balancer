This project allows you to create a load balancer, which acts as a reverse proxy for a certain amount of web servers, and splits the traffic between the servers.

If a server is down or has problems of it's own, the load balancer will take this into account and send the clients on other servers that are alive and well.

These servers are defined in the file "tests/ressources/config.json".

You should update this file with your IP address (the one from the machine that acts as load balancer), in the first "ip" field.

You can also define a port on this machine which will be the entry point for any client.

Then, there is a part for parameters such as:
 - "active_health_check_interval", which will be the number of seconds to wait between each health check
   (a health check is for asking each server if they are alive and well, in order to keep track of this).
 - "active_health_check_path", which is the url on each server that is made especially for the load balancer to monitor.
 - "rate_limit_window_size", which will define how many seconds pass by before a blacklisted client can regain access (still in dev for the moment).
 - "max_requests_per_window", which is the max number of requests made by one client before they get blacklisted.

If you don't have any web servers for starters, no problem !
You can use our test servers and still run the load balancer :

```
cd servers
./start.bash
cd ..
```

Once you are set with running web servers, you can add each of them in the "tests/ressources/config.json" file.
The "servers" parameter contains a list of servers, update it with your own (ip, port).

All done ? Starting the load balancer is then very simple :

```
cargo run
```

enjoy :)
