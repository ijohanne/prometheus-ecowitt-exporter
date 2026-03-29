# prometheus-ecowitt-exporter

Prometheus exporter for Ecowitt weather stations, written in Rust.

## Table of Contents

- [Overview](#overview)
- [Multi-station support](#multi-station-support)
- [Supported hardware](#supported-hardware)
- [CLI options](#cli-options)
- [How to configure your weather station](#how-to-configure-your-weather-station)
- [Deployment](#deployment)
  - [NixOS](#nixos)
  - [Docker](#docker)
  - [Binary](#binary)
- [Grafana](#grafana)
- [Testing](#testing)
- [Polling frequency](#polling-frequency)

## Overview

Ecowitt weather stations can push metrics to a custom HTTP endpoint using the Ecowitt protocol.
This exporter receives those POST requests and presents the data as Prometheus metrics.

The exporter runs on a single HTTP port (default `8088`) and provides:

- `GET /` - version info
- `POST /report/{station}` - where the Ecowitt weather station POSTs its data
- `GET /metrics` - where Prometheus scrapes metrics

Data flow:

1. Ecowitt sensors submit readings to the gateway via RF
2. The gateway aggregates data and POSTs to `/report/{station}`
3. Prometheus scrapes `/metrics`
4. Grafana queries Prometheus for visualisation

## Multi-station support

Each gateway posts to a unique path, e.g. `/report/garden` or `/report/rooftop`.
The station name from the URL is added as a `station` label on every metric,
so a single exporter instance can serve multiple gateways without collisions.

## Supported hardware

Most Ecowitt weather stations and sensors should work. The following have been tested:

### Weather stations

- WS2910 Weather Station
- GW1100 Wi-Fi Gateway

### Sensors

- WS69 Wireless 7-in-1 Outdoor Sensor Array
- WS90 7-in-1 Outdoor Anti-vibration Haptic Sensor Array
- WH41/WH43 PM2.5 Air Quality Sensor
- WH57 Outdoor Lightning Sensor
- WN31/WH31 Multi-Channel Temperature & Humidity Sensor
- WN32/WH32 Single-Channel Temperature & Humidity Sensor
- WN36 Floating Pool Temperature Sensor
- WH51 Soil Moisture Meter

## CLI options

All configuration is via command-line flags. Metric/SI units are the default.

| Flag | Default | Choices | Description |
|------|---------|---------|-------------|
| `--listen-port` | `8088` | | Port to listen on |
| `--listen-address` | `0.0.0.0` | | Address to bind to |
| `--temperature-unit` | `c` | `c`, `f`, `k` | Celsius, Fahrenheit, or Kelvin |
| `--pressure-unit` | `hpa` | `hpa`, `inhg`, `mmhg` | Hectopascals, inches Hg, or mm Hg |
| `--wind-unit` | `kmh` | `kmh`, `mph`, `ms`, `knots`, `fps` | Wind speed unit |
| `--rain-unit` | `mm` | `mm`, `in` | Rainfall unit |
| `--distance-unit` | `km` | `km`, `mi` | Lightning distance unit |
| `--irradiance-unit` | `wm2` | `wm2`, `lx`, `fc` | Solar irradiance unit |
| `--aqi-standard` | `uk` | `uk`, `epa`, `mep`, `nepm` | Air Quality Index standard |
| `--outdoor-location` | | | Label for outdoor sensor location |
| `--indoor-location` | | | Label for indoor sensor location |
| `--temp1-location` .. `--temp8-location` | | | Label for channel 1-8 temperature sensors |
| `--debug` | `false` | | Enable debug logging |

## How to configure your weather station

Use the WSView Plus app to configure each gateway. Go into the device, scroll to Customized:

- Customized: Enable
- Protocol: Ecowitt
- Server IP / Hostname: the IP or hostname of this exporter
- Path: `/report/mystationname` (choose a unique name per gateway)
- Port: `8088`
- Upload interval: `60`

## Deployment

### NixOS

This project provides a NixOS flake with a module. Add it to your flake inputs and enable the service:

```nix
{
  services.prometheus-ecowitt-exporter = {
    enable = true;
    port = 8088;
    temperatureUnit = "c";
    windUnit = "kmh";
    outdoorLocation = "Garden";
    tempLocations = { "1" = "Greenhouse"; "2" = "Garage"; };
  };
}
```

Optional opt-in features:
- `enableLocalScraping` - automatically configure local Prometheus to scrape this exporter
- `enableGrafanaDashboard` - provision the included Grafana dashboard

### Docker

```console
docker build -t prometheus-ecowitt-exporter .
docker run -d --rm -p 8088:8088 prometheus-ecowitt-exporter \
  --temperature-unit c --wind-unit kmh
```

### Binary

```console
cargo build --release
./target/release/prometheus-ecowitt-exporter --temperature-unit c --outdoor-location Garden
```

## Grafana

An accompanying [Grafana dashboard](grafana/EcowittWeatherStation.json) is included.
Import it into your Grafana instance and select your Prometheus datasource.

Note: dashboard units are currently hardcoded and must be manually adjusted
to match your exporter's unit configuration.

## Testing

Sample data from an Ecowitt GW1100A is provided in `data.txt`.

Simulate a POST from a weather station:

```console
curl -d @data.txt -X POST http://127.0.0.1:8088/report/test
```

View the metrics:

```console
curl http://127.0.0.1:8088/metrics
```

## Polling frequency

Ecowitt sensors have hard-coded RF reporting intervals:

| Device | Reporting interval |
|--------|-------------------|
| WS69 Sensor Array | 16 seconds |
| WS90 Haptic Sensor Array | 8.8 seconds |
| WH41/WH43 PM2.5 Air Quality Sensor | 10 minutes |
| WH57 Lightning Sensor | 79 seconds |
| WN31/WH31 Temperature & Humidity Sensor | 61 seconds |
| WN32/WH32 Temperature & Humidity Sensor | 64 seconds |
| WN36 Floating Pool Temperature Sensor | 60 seconds |
| WH51 Soil Moisture Meter | 70 seconds |

The gateway upload interval (default 60 seconds) and Prometheus scrape interval
should be configured to match.
