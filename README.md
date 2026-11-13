# What is this?

This repo is a Rust clone of the AviationApi `/charts` endpoint. See docs from
AviationAPI [here](https://docs.aviationapi.com/). This clone supports FAA LID and ICAO airport ids as well as
the `group` query param as specified by AviationAPI.

Unlike AviationAPI, this clone does not re-host the chart PDFs. Instead, the API returns links to the FAA-hosted PDFs.

# Additional Features

This version includes the following features that are a superset to the AviationAPI `/charts` functionality

- Retrieve a single chart with `/charts/{airport id}/{search term}`. This will redirect to the first FAA-hosted chart
  PDF
  that includes the search term in the chart's name (case-insensitive)
- Host static charts at `/charts/static/{static file}`, served from the `assets` directory.
  The Dockerfile will copy `assets` in the deployment

# MCP Support

- `get_charts` maps to `/charts/{airport id}`
- Served at `/mcp` via Streamable HTTP (MCP transport after `2025-03-26`)

## Breaking change (rmcp 3.x)

The legacy HTTP+SSE transport (`GET /sse` + `POST /message`) was removed from
`rmcp` and is no longer exposed by this server. MCP clients must use Streamable
HTTP against `/mcp`.

## Host allowlist

`rmcp` validates the inbound `Host` header to mitigate DNS rebinding. By default
only loopback hosts are accepted: `localhost`, `127.0.0.1`, and `::1` (any port).
Requests with any other `Host` value receive HTTP 403 on `/mcp` only; the REST
API is unaffected.

Local access (including Docker Compose port publishes such as
`localhost:8999`) works with the default. Public or reverse-proxied deployments
must configure an explicit host allowlist in the MCP server config (for example
`StreamableHttpServerConfig::with_allowed_hosts([...])`) for each expected
hostname, or terminate TLS/HTTP behind a proxy that rejects unexpected `Host`
headers. Do not disable host validation unless an upstream proxy enforces it.
