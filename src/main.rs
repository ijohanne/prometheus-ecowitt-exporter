use anyhow::Result;
use clap::Parser;
use prometheus::{GaugeVec, Opts, Registry};
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use warp::{Filter, Rejection, Reply};

// Unit conversion functions

fn mph2kmh(mph: f64) -> f64 {
    mph * 1.60934
}

fn mph2ms(mph: f64) -> f64 {
    mph / 2.237
}

fn mph2kts(mph: f64) -> f64 {
    mph / 1.151
}

fn mph2fps(mph: f64) -> f64 {
    mph * 1.467
}

fn in2mm(inches: f64) -> f64 {
    inches * 25.4
}

fn km2mi(km: f64) -> f64 {
    km / 1.60934
}

fn inhg2hpa(inhg: f64) -> f64 {
    inhg * 33.8639
}

fn inhg2mmhg(inhg: f64) -> f64 {
    inhg * 25.4
}

fn wm22lux(wm2: f64) -> f64 {
    wm2 / 0.0079
}

fn wm22fc(wm2: f64) -> f64 {
    wm2 * 6.345
}

fn f2c(f: f64) -> f64 {
    (f - 32.0) * 5.0 / 9.0
}

fn f2k(f: f64) -> f64 {
    (f - 32.0) * 5.0 / 9.0 + 273.15
}

fn aqi_uk(concentration: f64) -> f64 {
    if concentration < 12.0 {
        1.0
    } else if concentration < 24.0 {
        2.0
    } else if concentration < 36.0 {
        3.0
    } else if concentration < 42.0 {
        4.0
    } else if concentration < 48.0 {
        5.0
    } else if concentration < 54.0 {
        6.0
    } else if concentration < 59.0 {
        7.0
    } else if concentration < 65.0 {
        8.0
    } else if concentration < 71.0 {
        9.0
    } else {
        10.0
    }
}

fn aqi_nepm(concentration: f64) -> f64 {
    (100.0 * concentration / 25.0).round()
}

fn aqi_epa(concentration: f64) -> f64 {
    let breakpoints = &[
        (0.0, 12.0, 0.0, 50.0),
        (12.1, 35.4, 51.0, 100.0),
        (35.5, 55.4, 101.0, 150.0),
        (55.5, 150.4, 151.0, 200.0),
        (150.5, 250.4, 201.0, 300.0),
        (250.5, 350.4, 301.0, 400.0),
        (350.5, 500.4, 401.0, 500.0),
    ];
    for &(c_low, c_high, i_low, i_high) in breakpoints {
        if concentration >= c_low && concentration <= c_high {
            return (i_high - i_low) / (c_high - c_low) * (concentration - c_low) + i_low;
        }
    }
    500.0
}

fn aqi_mep(concentration: f64) -> f64 {
    let breakpoints = &[
        (0.0, 35.0, 0.0, 50.0),
        (35.0, 75.0, 50.0, 100.0),
        (75.0, 115.0, 100.0, 150.0),
        (115.0, 150.0, 150.0, 200.0),
        (150.0, 250.0, 200.0, 300.0),
        (250.0, 350.0, 300.0, 400.0),
        (350.0, 500.0, 400.0, 500.0),
    ];
    for &(c_low, c_high, i_low, i_high) in breakpoints {
        if concentration >= c_low && concentration <= c_high {
            return (i_high - i_low) / (c_high - c_low) * (concentration - c_low) + i_low;
        }
    }
    500.0
}

#[allow(clippy::doc_markdown)]
fn mph2beaufort(speed: f64) -> f64 {
    if speed <= 1.0 {
        0.0
    } else if speed <= 3.0 {
        1.0
    } else if speed <= 7.0 {
        2.0
    } else if speed <= 12.0 {
        3.0
    } else if speed <= 18.0 {
        4.0
    } else if speed <= 24.0 {
        5.0
    } else if speed <= 31.0 {
        6.0
    } else if speed <= 38.0 {
        7.0
    } else if speed <= 46.0 {
        8.0
    } else if speed <= 54.0 {
        9.0
    } else if speed <= 63.0 {
        10.0
    } else if speed <= 73.0 {
        11.0
    } else {
        12.0
    }
}

fn calculate_aqi(standard: &str, value: f64) -> f64 {
    match standard {
        "epa" => aqi_epa(value),
        "mep" => aqi_mep(value),
        "nepm" => aqi_nepm(value),
        _ => aqi_uk(value),
    }
}

// Prometheus metrics container

#[derive(Clone)]
struct Metrics {
    temp: GaugeVec,
    humidity: GaugeVec,
    winddir: GaugeVec,
    uv: GaugeVec,
    pm25: GaugeVec,
    aqi: GaugeVec,
    batterystatus: GaugeVec,
    batterylevel: GaugeVec,
    batteryvoltage: GaugeVec,
    solarradiation: GaugeVec,
    barom: GaugeVec,
    vpd: GaugeVec,
    wind: GaugeVec,
    wind_beaufort: GaugeVec,
    rain: GaugeVec,
    lightning: GaugeVec,
    lightning_num: GaugeVec,
    lightning_time: GaugeVec,
    ws90: GaugeVec,
    soilmoisture: GaugeVec,
    registry: Registry,
}

impl Metrics {
    #[allow(clippy::too_many_lines)]
    fn new() -> Self {
        let registry = Registry::new();

        let temp = GaugeVec::new(
            Opts::new("ecowitt_temp", "Temperature"),
            &["station", "sensor", "unit", "location"],
        )
        .expect("metric can be created");

        let humidity = GaugeVec::new(
            Opts::new("ecowitt_humidity", "Relative humidity"),
            &["station", "sensor", "unit", "location"],
        )
        .expect("metric can be created");

        let winddir = GaugeVec::new(Opts::new("ecowitt_winddir", "Wind direction"), &["station"])
            .expect("metric can be created");

        let uv = GaugeVec::new(Opts::new("ecowitt_uv", "UV index"), &["station"])
            .expect("metric can be created");

        let pm25 = GaugeVec::new(
            Opts::new("ecowitt_pm25", "PM2.5 concentration"),
            &["station", "series", "sensor", "unit"],
        )
        .expect("metric can be created");

        let aqi =
            GaugeVec::new(Opts::new("ecowitt_aqi", "Air quality index"), &["station", "standard"])
                .expect("metric can be created");

        let batterystatus = GaugeVec::new(
            Opts::new("ecowitt_batterystatus", "Battery status"),
            &["station", "sensor"],
        )
        .expect("metric can be created");

        let batterylevel = GaugeVec::new(
            Opts::new("ecowitt_batterylevel", "Battery level"),
            &["station", "sensor"],
        )
        .expect("metric can be created");

        let batteryvoltage = GaugeVec::new(
            Opts::new("ecowitt_batteryvoltage", "Battery voltage"),
            &["station", "sensor", "unit"],
        )
        .expect("metric can be created");

        let solarradiation = GaugeVec::new(
            Opts::new("ecowitt_solarradiation", "Solar irradiance"),
            &["station", "unit"],
        )
        .expect("metric can be created");

        let barom =
            GaugeVec::new(Opts::new("ecowitt_barom", "Barometer"), &["station", "sensor", "unit"])
                .expect("metric can be created");

        let vpd = GaugeVec::new(
            Opts::new("ecowitt_vpd", "Vapour pressure deficit"),
            &["station", "unit"],
        )
        .expect("metric can be created");

        let wind = GaugeVec::new(
            Opts::new("ecowitt_windspeed", "Wind speed"),
            &["station", "sensor", "unit"],
        )
        .expect("metric can be created");

        let wind_beaufort = GaugeVec::new(
            Opts::new("ecowitt_windspeed_beaufort", "Wind Beaufort scale"),
            &["station"],
        )
        .expect("metric can be created");

        let rain =
            GaugeVec::new(Opts::new("ecowitt_rain", "Rainfall"), &["station", "sensor", "unit"])
                .expect("metric can be created");

        let lightning = GaugeVec::new(
            Opts::new("ecowitt_lightning", "Lightning distance"),
            &["station", "unit"],
        )
        .expect("metric can be created");

        let lightning_num = GaugeVec::new(
            Opts::new("ecowitt_lightning_num", "Lightning daily count"),
            &["station"],
        )
        .expect("metric can be created");

        let lightning_time = GaugeVec::new(
            Opts::new("ecowitt_lightning_time", "Lightning last strike"),
            &["station"],
        )
        .expect("metric can be created");

        let ws90 = GaugeVec::new(
            Opts::new("ecowitt_wh90", "WS90 electrical energy stored"),
            &["station", "sensor", "unit"],
        )
        .expect("metric can be created");

        let soilmoisture = GaugeVec::new(
            Opts::new("ecowitt_soilmoisture", "Soil moisture"),
            &["station", "sensor", "unit"],
        )
        .expect("metric can be created");

        macro_rules! register {
            ($($metric:expr),+ $(,)?) => {
                $(registry.register(Box::new($metric.clone())).expect("collector can be registered");)+
            };
        }
        register!(
            temp,
            humidity,
            winddir,
            uv,
            pm25,
            aqi,
            batterystatus,
            batterylevel,
            batteryvoltage,
            solarradiation,
            barom,
            vpd,
            wind,
            wind_beaufort,
            rain,
            lightning,
            lightning_num,
            lightning_time,
            ws90,
            soilmoisture,
        );

        Self {
            temp,
            humidity,
            winddir,
            uv,
            pm25,
            aqi,
            batterystatus,
            batterylevel,
            batteryvoltage,
            solarradiation,
            barom,
            vpd,
            wind,
            wind_beaufort,
            rain,
            lightning,
            lightning_num,
            lightning_time,
            ws90,
            soilmoisture,
            registry,
        }
    }
}

// Configuration from environment / CLI

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about = "Prometheus exporter for Ecowitt weather stations")]
struct Args {
    #[clap(long, default_value = "8088")]
    listen_port: u16,

    #[clap(long, default_value = "0.0.0.0")]
    listen_address: String,

    #[clap(long, default_value = "c")]
    temperature_unit: String,

    #[clap(long, default_value = "hpa")]
    pressure_unit: String,

    #[clap(long, default_value = "kmh")]
    wind_unit: String,

    #[clap(long, default_value = "mm")]
    rain_unit: String,

    #[clap(long, default_value = "km")]
    distance_unit: String,

    #[clap(long, default_value = "wm2")]
    irradiance_unit: String,

    #[clap(long, default_value = "epa")]
    aqi_standard: String,

    #[clap(long, default_value = "ecowitt")]
    station_id: String,

    #[clap(long)]
    outdoor_location: Option<String>,

    #[clap(long)]
    indoor_location: Option<String>,

    #[clap(long)]
    temp1_location: Option<String>,

    #[clap(long)]
    temp2_location: Option<String>,

    #[clap(long)]
    temp3_location: Option<String>,

    #[clap(long)]
    temp4_location: Option<String>,

    #[clap(long)]
    temp5_location: Option<String>,

    #[clap(long)]
    temp6_location: Option<String>,

    #[clap(long)]
    temp7_location: Option<String>,

    #[clap(long)]
    temp8_location: Option<String>,

    #[clap(long)]
    debug: bool,
}

impl Args {
    fn temp_location(&self, channel: char) -> Option<&str> {
        match channel {
            '1' => self.temp1_location.as_deref(),
            '2' => self.temp2_location.as_deref(),
            '3' => self.temp3_location.as_deref(),
            '4' => self.temp4_location.as_deref(),
            '5' => self.temp5_location.as_deref(),
            '6' => self.temp6_location.as_deref(),
            '7' => self.temp7_location.as_deref(),
            '8' => self.temp8_location.as_deref(),
            _ => None,
        }
    }
}

struct AppState {
    metrics: Metrics,
    config: Args,
}

// WS90 piezo rain sensor mappings
fn rain_piezo_maps() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("rrain_piezo", "rainrate");
    m.insert("erain_piezo", "eventrain");
    m.insert("hrain_piezo", "hourlyrain");
    m.insert("drain_piezo", "dailyrain");
    m.insert("wrain_piezo", "weeklyrain");
    m.insert("mrain_piezo", "monthlyrain");
    m.insert("yrain_piezo", "yearlyrain");
    m
}

fn convert_temperature(value: f64, unit: &str) -> f64 {
    match unit {
        "c" => f2c(value),
        "k" => f2k(value),
        _ => value,
    }
}

fn convert_pressure(value: f64, unit: &str) -> f64 {
    match unit {
        "hpa" => inhg2hpa(value),
        "mmhg" => inhg2mmhg(value),
        _ => value,
    }
}

fn convert_wind(value: f64, unit: &str) -> f64 {
    match unit {
        "kmh" => mph2kmh(value),
        "ms" => mph2ms(value),
        "knots" => mph2kts(value),
        "fps" => mph2fps(value),
        _ => value,
    }
}

fn convert_rain(value: f64, unit: &str) -> f64 {
    match unit {
        "mm" => in2mm(value),
        _ => value,
    }
}

fn convert_irradiance(value: f64, unit: &str) -> f64 {
    match unit {
        "lx" => wm22lux(value),
        "fc" => wm22fc(value),
        _ => value,
    }
}

#[allow(clippy::too_many_lines)]
fn process_report(state: &AppState, station: &str, data: &HashMap<String, String>) {
    let config = &state.config;
    let metrics = &state.metrics;
    let ch_re = Regex::new(r"(ch\d)$").expect("valid regex");
    let rainmaps = rain_piezo_maps();
    let battery_level_keys = ["wh57batt", "pm25batt1", "pm25batt2"];

    for (key, raw_value) in data {
        let key = key.as_str();

        if config.debug {
            println!("[{station}] Received raw value {key}: {raw_value}");
        }

        // Ignore these fields
        if matches!(key, "PASSKEY" | "dateutc" | "runtime" | "heap") {
            continue;
        }

        // Info fields - skip in Rust version as prometheus crate doesn't have Info type
        // The Python version uses prometheus_client Info, but these are rarely queried
        if matches!(key, "stationtype" | "freq" | "model" | "interval") {
            continue;
        }

        // No conversions needed
        if key == "winddir" {
            if let Ok(v) = raw_value.parse::<f64>() {
                metrics.winddir.with_label_values(&[station]).set(v);
            }
            continue;
        }
        if key == "uv" {
            if let Ok(v) = raw_value.parse::<f64>() {
                metrics.uv.with_label_values(&[station]).set(v);
            }
            continue;
        }
        if key == "lightning_num" {
            if let Ok(v) = raw_value.parse::<f64>() {
                metrics.lightning_num.with_label_values(&[station]).set(v);
            }
            continue;
        }
        if key == "lightning_time" {
            if let Ok(v) = raw_value.parse::<f64>() {
                metrics.lightning_time.with_label_values(&[station]).set(v);
            }
            continue;
        }

        // WS90 capacitor voltage
        if key == "ws90cap_volt" {
            if let Ok(v) = raw_value.parse::<f64>() {
                metrics.ws90.with_label_values(&[station, key, "volt"]).set(v);
            }
            continue;
        }

        // Battery status & levels
        if key.contains("batt") {
            if let Ok(v) = raw_value.parse::<f64>() {
                if battery_level_keys.contains(&key) {
                    metrics.batterylevel.with_label_values(&[station, key]).set(v);
                } else if key.starts_with("soil") || key.starts_with("ws90") {
                    metrics.batteryvoltage.with_label_values(&[station, key, "volt"]).set(v);
                } else {
                    metrics.batterystatus.with_label_values(&[station, key]).set(v);
                }
            }
            continue;
        }

        // Soil moisture
        if key.starts_with("soilmoisture") {
            if let Ok(v) = raw_value.parse::<f64>() {
                metrics.soilmoisture.with_label_values(&[station, key, "percent"]).set(v);
            }
            continue;
        }

        // Skip soilad keys
        if key.starts_with("soilad") {
            continue;
        }

        // PM2.5
        if key.starts_with("pm25") && !key.starts_with("pm25batt") {
            // Check for invalid readings when battery is low
            let skip_ch1 = data.get("pm25batt1").is_some_and(|b| b == "1")
                && data.get("pm25_ch1").is_some_and(|v| v == "1000");
            let skip_ch2 = data.get("pm25batt2").is_some_and(|b| b == "1")
                && data.get("pm25_ch2").is_some_and(|v| v == "1000");
            if skip_ch1 || skip_ch2 {
                if config.debug {
                    println!("[{station}] Drop erroneous PM25 reading {key}: {raw_value}");
                }
                continue;
            }

            if let Ok(v) = raw_value.parse::<f64>() {
                // Drop pm25_ prefix
                let stripped = key.replace("pm25_", "");

                // Get sensor channel suffix
                if let Some(caps) = ch_re.captures(&stripped) {
                    let sensor = caps.get(1).expect("capture group exists").as_str();
                    let remainder = ch_re.replace(&stripped, "").to_string();

                    let series = if remainder.starts_with("avg_24h") {
                        "avg_24h"
                    } else {
                        "realtime"
                    };

                    metrics
                        .pm25
                        .with_label_values(&[station, series, sensor, "\u{03bc}gm3"])
                        .set(v);

                    // Calculate AQI from 24h average
                    if remainder.starts_with("avg_24h") {
                        let aqi_val = calculate_aqi(&config.aqi_standard, v);
                        metrics
                            .aqi
                            .with_label_values(&[station, &config.aqi_standard])
                            .set(aqi_val);
                    }
                }
            }
            continue;
        }

        // Humidity
        if key.starts_with("humidity") {
            if let Ok(v) = raw_value.parse::<f64>() {
                let (label, location) = if key == "humidity" {
                    (
                        "outdoor".to_string(),
                        config.outdoor_location.as_deref().unwrap_or("outdoor").to_string(),
                    )
                } else if key == "humidityin" {
                    (
                        "indoor".to_string(),
                        config.indoor_location.as_deref().unwrap_or("indoor").to_string(),
                    )
                } else {
                    let ch = key.chars().last().unwrap_or('0');
                    let label = format!("ch{ch}");
                    let location = config.temp_location(ch).unwrap_or(&label).to_string();
                    (label, location)
                };
                metrics.humidity.with_label_values(&[station, &label, "percent", &location]).set(v);
            }
            continue;
        }

        // Solar irradiance
        if key == "solarradiation" {
            if let Ok(v) = raw_value.parse::<f64>() {
                let converted = convert_irradiance(v, &config.irradiance_unit);
                metrics
                    .solarradiation
                    .with_label_values(&[station, &config.irradiance_unit])
                    .set(converted);
            }
            continue;
        }

        // Temperature
        if key.starts_with("temp") {
            if let Ok(v) = raw_value.parse::<f64>() {
                // Strip trailing 'f'
                let stripped = &key[..key.len() - 1];
                let converted = convert_temperature(v, &config.temperature_unit);

                let (label, location) = if stripped == "tempin" {
                    (
                        "indoor".to_string(),
                        config.indoor_location.as_deref().unwrap_or("indoor").to_string(),
                    )
                } else if stripped == "temp" {
                    (
                        "outdoor".to_string(),
                        config.outdoor_location.as_deref().unwrap_or("outdoor").to_string(),
                    )
                } else {
                    let ch = stripped.chars().last().unwrap_or('0');
                    let label = format!("ch{ch}");
                    let location = config.temp_location(ch).unwrap_or(&label).to_string();
                    (label, location)
                };

                metrics
                    .temp
                    .with_label_values(&[station, &label, &config.temperature_unit, &location])
                    .set(converted);
            }
            continue;
        }

        // Barometric pressure
        if key.starts_with("barom") {
            if let Ok(v) = raw_value.parse::<f64>() {
                let converted = convert_pressure(v, &config.pressure_unit);
                // Remove 'in' suffix
                let stripped = &key[..key.len() - 2];
                let label = if stripped == "baromrel" {
                    "relative"
                } else {
                    "absolute"
                };
                metrics
                    .barom
                    .with_label_values(&[station, label, &config.pressure_unit])
                    .set(converted);
            }
            continue;
        }

        // Vapour pressure deficit
        if key == "vpd" {
            if let Ok(v) = raw_value.parse::<f64>() {
                let converted = convert_pressure(v, &config.pressure_unit);
                metrics.vpd.with_label_values(&[station, &config.pressure_unit]).set(converted);
            }
            continue;
        }

        // Wind speed
        if matches!(key, "windspeedmph" | "windgustmph" | "maxdailygust") {
            if let Ok(v) = raw_value.parse::<f64>() {
                let converted = convert_wind(v, &config.wind_unit);

                if key == "windspeedmph" {
                    let beaufort = mph2beaufort(v);
                    metrics.wind_beaufort.with_label_values(&[station]).set(beaufort);
                }

                let sensor = if key == "maxdailygust" {
                    "maxdailygust".to_string()
                } else {
                    key[..key.len() - 3].to_string()
                };
                metrics
                    .wind
                    .with_label_values(&[station, &sensor, &config.wind_unit])
                    .set(converted);
            }
            continue;
        }

        // WS90 piezo rain sensors
        if key.ends_with("piezo") {
            if let Ok(v) = raw_value.parse::<f64>() {
                if rainmaps.contains_key(key) {
                    let converted = convert_rain(v, &config.rain_unit);
                    metrics
                        .rain
                        .with_label_values(&[station, key, &config.rain_unit])
                        .set(converted);
                }
            }
            continue;
        }

        // Rainfall
        if key.contains("rain") {
            if let Ok(v) = raw_value.parse::<f64>() {
                let converted = convert_rain(v, &config.rain_unit);
                // Remove 'in' suffix, then 'rain' substring
                let stripped = &key[..key.len() - 2];
                let sensor = stripped.replace("rain", "");
                metrics
                    .rain
                    .with_label_values(&[station, &sensor, &config.rain_unit])
                    .set(converted);
            }
            continue;
        }

        // Lightning distance
        if key == "lightning" {
            if let Ok(v) = raw_value.parse::<f64>() {
                let converted = if config.distance_unit == "mi" {
                    km2mi(v)
                } else {
                    v
                };
                metrics
                    .lightning
                    .with_label_values(&[station, &config.distance_unit])
                    .set(converted);
            }
        }
    }
}

fn with_state(
    state: Arc<Mutex<AppState>>,
) -> impl Filter<Extract = (Arc<Mutex<AppState>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

async fn metrics_handler(state: Arc<Mutex<AppState>>) -> Result<impl Reply, Rejection> {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();

    let state = state.lock().expect("lock not poisoned");
    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&state.metrics.registry.gather(), &mut buffer) {
        eprintln!("could not encode custom metrics: {e}");
    }
    let res = String::from_utf8(buffer).unwrap_or_default();
    Ok(res)
}

async fn report_handler(
    station: String,
    state: Arc<Mutex<AppState>>,
    body: String,
) -> Result<impl Reply, Rejection> {
    let data: HashMap<String, String> = url::form_urlencoded::parse(body.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    let state = state.lock().expect("lock not poisoned");
    process_report(&state, &station, &data);

    Ok(warp::reply::with_status("OK", warp::http::StatusCode::OK))
}

async fn version_handler() -> Result<impl Reply, Rejection> {
    Ok("Ecowitt Exporter\n")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("Ecowitt Exporter");
    println!("================");
    println!("Configuration:");
    println!("  DEBUG:            {}", args.debug);
    println!("  TEMPERATURE_UNIT: {}", args.temperature_unit);
    println!("  PRESSURE_UNIT:    {}", args.pressure_unit);
    println!("  WIND_UNIT:        {}", args.wind_unit);
    println!("  RAIN_UNIT:        {}", args.rain_unit);
    println!("  DISTANCE_UNIT:    {}", args.distance_unit);
    println!("  IRRADIANCE_UNIT:  {}", args.irradiance_unit);
    println!("  AQI STANDARD:     {}", args.aqi_standard);
    println!("  STATION_ID:       {}", args.station_id);

    let metrics = Metrics::new();
    let state = Arc::new(Mutex::new(AppState {
        metrics,
        config: args.clone(),
    }));

    let version_route = warp::path::end().and(warp::get()).and_then(version_handler);

    let metrics_route = warp::path!("metrics")
        .and(warp::get())
        .and(with_state(state.clone()))
        .and_then(metrics_handler);

    let report_route =
        warp::path!("report" / String)
            .and(warp::post())
            .and(with_state(state))
            .and(warp::body::content_length_limit(1024 * 64))
            .and(warp::body::bytes().map(|bytes: warp::hyper::body::Bytes| {
                String::from_utf8_lossy(&bytes).into_owned()
            }))
            .and_then(report_handler);

    let routes = version_route.or(metrics_route).or(report_route);

    let addr: std::net::SocketAddr = format!("{}:{}", args.listen_address, args.listen_port)
        .parse()
        .expect("valid listen address");

    println!("Listening on {addr}");
    warp::serve(routes).run(addr).await;

    Ok(())
}
