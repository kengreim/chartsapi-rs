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
