
# Orderbook Pull


```

Run gRPC server:

```
cargo run --bin ordermaster-server
```

Client
-----

Connects to the gRPC server and streams the orderbook summary.

```
USAGE:
    ordermaster-dashboard [OPTIONS]

OPTIONS:
    -p, --port <PORT>    (Optional) Port number of the gRPC server. Default: 50051
```

Run gRPC client:

```
cargo run --bin ordermaster-dashboard



