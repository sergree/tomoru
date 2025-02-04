# IP Request Counter

Simple service that counts HTTP requests per IP address.

## Usage

0. Run tests:
```bash
cargo test
```

1. Run the service:
```bash
cargo run
```

2. Test with requests:
```bash
# If running locally:
curl http://127.0.0.1:3000/ping

# If running on remote server, replace with your server's IP:
curl http://SERVER_IP:3000/ping
```

The service will display request counts per IP address every second.
