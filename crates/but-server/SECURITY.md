# Security: Localhost-Only Connections

## Overview

The `but-server` implements a security middleware that ensures all incoming connections originate from localhost only. This prevents unauthorized remote access to the server.

## Implementation

The server uses an Axum middleware (`localhost_only_middleware`) that:

1. Extracts the client's socket address using `ConnectInfo<SocketAddr>`
2. Checks if the IP address is a loopback address (127.0.0.1 for IPv4 or ::1 for IPv6)
3. Accepts the connection if it's from localhost
4. Rejects the connection with HTTP 403 Forbidden if it's from any other address

## Testing

To verify the security middleware:

1. Start the server:
   ```bash
   cargo run -p but-server
   ```

2. Test from localhost (should succeed):
   ```bash
   curl -i http://127.0.0.1:6978/
   ```

3. Attempt to connect from a non-localhost address (should fail with 403):
   - This can only be tested if the server is bound to a non-localhost address
   - By default, the server binds to 127.0.0.1 which already prevents remote connections at the TCP level
   - The middleware provides defense-in-depth even if the bind address is changed

## Configuration

The server listens on:
- Host: `BUTLER_HOST` environment variable (default: `127.0.0.1`)
- Port: `BUTLER_PORT` environment variable (default: `6978`)

**Security Note**: While the default bind address is `127.0.0.1`, the middleware ensures security even if the bind address is accidentally changed to `0.0.0.0` or another interface.

## Logs

Rejected connections are logged with a warning:
```
Rejected non-localhost connection from: <ip_address>
```
