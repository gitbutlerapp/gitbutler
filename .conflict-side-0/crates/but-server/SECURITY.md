# Security: Localhost-Only Connections

## Overview

The `but-server` implements a security middleware that ensures all incoming connections originate from localhost only. This prevents unauthorized remote access to the server.

## Implementation

The server uses an Axum middleware (`localhost_only_middleware`) that:

1. Extracts the client's socket address using `ConnectInfo<SocketAddr>`
2. Checks if the IP address is a loopback address (127.0.0.1 for IPv4 or ::1 for IPv6)
3. Accepts the connection if it's from localhost
4. Rejects the connection with HTTP 403 Forbidden if it's from any other address

When this passes, CORS handling additionally validates any `Origin` header that is present, allowing only origins with scheme `http`, host `localhost`, and an optional port (for example, `http://localhost` or `http://localhost:3000`), and rejecting other schemes or hosts.

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

# Considerations for Electron

Should the `but-server` be used in Electron, we will make sure that it's completely locked down and only usable by the Electron process it's bundled with.