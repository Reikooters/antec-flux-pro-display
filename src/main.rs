//! Shows CPU and GPU temperatures on the side-panel display of an Antec Flux Pro case.
//!
//! Temperatures come from libsensors, so any device with a hwmon driver can be shown.
//! Which sensors to use is configured in `/etc/antec-flux-pro-display/config.conf`.

mod usb;

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use sensors::{Chip, FeatureType, Sensors, Subfeature};

use usb::UsbDevice;

const CONFIG_PATH: &str = "/etc/antec-flux-pro-display/config.conf";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// An update that overruns by this much is taken to mean the machine was suspended.
const SUSPEND_SLACK: Duration = Duration::from_secs(2);
/// Exit status for a configuration or usage error, which a restart will not fix.
const EXIT_CONFIG: i32 = 2;
const EXIT_USAGE: i32 = 2;
const MIN_UPDATE_INTERVAL: Duration = Duration::from_millis(100);
const DEFAULT_UPDATE_INTERVAL: Duration = Duration::from_millis(1000);

const SAMPLE_CONFIG: &str = "\
# CPU device for temperature monitoring
cpu_device=k10temp
cpu_temp_type=tctl

# GPU device for temperature monitoring
gpu_device=amdgpu
gpu_temp_type=edge

# Update interval in milliseconds
update_interval=1000";

const USAGE: &str = "\
usage: antec-flux-pro-display [--list-sensors | --version | --help]

Runs until stopped, sending the configured CPU and GPU temperatures to the
Antec Flux Pro case display once per update interval.

  --list-sensors  print every temperature sensor libsensors can see, and exit
  -V, --version   print the version, and exit
  -h, --help      print this help, and exit";

/// Which libsensors reading to show in one position on the panel.
#[derive(Debug, Clone)]
struct SensorSpec {
    /// Chip name prefix, e.g. `k10temp` or `amdgpu`; compared lower-case.
    device: String,
    /// Feature label prefix, e.g. `tctl` or `edge`; compared lower-case.
    temp_type: String,
    /// Optional PCI vendor and device ID (hex) to tell apart chips that share a name.
    vendor_id: String,
    device_id: String,
}

impl SensorSpec {
    fn from_config(config: &HashMap<String, String>, prefix: &str) -> Result<Self> {
        let required = |key: &str| -> Result<String> {
            config
                .get(&format!("{prefix}_{key}"))
                .map(|v| v.to_lowercase())
                .with_context(|| format!("{prefix}_{key} is not set in {CONFIG_PATH}"))
        };
        let optional_id = |key: &str| -> String {
            config
                .get(&format!("{prefix}_{key}"))
                .map(|v| normalize_hex_id(&v.to_lowercase()).to_owned())
                .unwrap_or_default()
        };
        Ok(Self {
            device: required("device")?,
            temp_type: required("temp_type")?,
            vendor_id: optional_id("vendor_id"),
            device_id: optional_id("device_id"),
        })
    }

    fn pinned_to_pci_id(&self) -> bool {
        !self.vendor_id.is_empty() || !self.device_id.is_empty()
    }
}

impl fmt::Display for SensorSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.device, self.temp_type)?;
        if self.pinned_to_pci_id() {
            fn or_any(id: &str) -> &str {
                if id.is_empty() { "*" } else { id }
            }
            write!(
                f,
                " [{}:{}]",
                or_any(&self.vendor_id),
                or_any(&self.device_id)
            )?;
        }
        Ok(())
    }
}

/// `0x1002` and `1002` both become `1002` (callers lower-case first, so `0X` works
/// too); leading zeros are dropped so that `0522` and `522` compare equal. An ID
/// that is all zeros becomes empty, which matches anything.
fn normalize_hex_id(id: &str) -> &str {
    let id = id.trim();
    let id = id.strip_prefix("0x").unwrap_or(id);
    id.trim_start_matches('0')
}

#[derive(Debug)]
struct Config {
    cpu: SensorSpec,
    gpu: SensorSpec,
    update_interval: Duration,
}

impl Config {
    fn load() -> Result<Self> {
        let text =
            fs::read_to_string(CONFIG_PATH).with_context(|| format!("reading {CONFIG_PATH}"))?;
        Self::parse(&text)
    }

    fn parse(text: &str) -> Result<Self> {
        let map: HashMap<String, String> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
            .collect();

        let update_interval = match map.get("update_interval") {
            None => DEFAULT_UPDATE_INTERVAL,
            Some(value) => {
                let millis: u64 = value.parse().with_context(|| {
                    format!("update_interval={value:?} is not a number of milliseconds")
                })?;
                let requested = Duration::from_millis(millis);
                if requested < MIN_UPDATE_INTERVAL {
                    eprintln!(
                        "update_interval={millis} is below the minimum; using {} ms",
                        MIN_UPDATE_INTERVAL.as_millis()
                    );
                }
                requested.max(MIN_UPDATE_INTERVAL)
            }
        };

        Ok(Self {
            cpu: SensorSpec::from_config(&map, "cpu")?,
            gpu: SensorSpec::from_config(&map, "gpu")?,
            update_interval,
        })
    }
}

/// A temperature reading that matched a `SensorSpec`.
struct Match {
    input: Subfeature,
    /// `chip label`, e.g. `k10temp-pci-00c3 Tctl`, for logging.
    description: String,
}

/// One position on the panel and the sensor bound to it.
struct Slot {
    name: &'static str,
    spec: SensorSpec,
    bound: Option<Match>,
    warned_ambiguous: bool,
}

impl Slot {
    fn new(name: &'static str, spec: SensorSpec) -> Self {
        Self {
            name,
            spec,
            bound: None,
            warned_ambiguous: false,
        }
    }

    fn read(&self) -> Option<f64> {
        self.bound.as_ref().and_then(|m| m.input.get_value().ok())
    }

    fn bind(&mut self, sensors: &Sensors) {
        let mut matches = find_matches(sensors, &self.spec);
        if matches.len() > 1 && !self.spec.pinned_to_pci_id() && !self.warned_ambiguous {
            let candidates: Vec<&str> = matches.iter().map(|m| m.description.as_str()).collect();
            eprintln!(
                "{} sensor {} matches more than one chip ({}); using the first. \
                 Pin one with {}_vendor_id and {}_device_id.",
                self.name,
                self.spec,
                candidates.join(", "),
                self.name.to_lowercase(),
                self.name.to_lowercase()
            );
            self.warned_ambiguous = true;
        }
        self.bound = if matches.is_empty() {
            None
        } else {
            Some(matches.swap_remove(0))
        };
        match &self.bound {
            Some(m) => println!(
                "{} sensor {}: using {}",
                self.name, self.spec, m.description
            ),
            None => eprintln!(
                "{} sensor {} not found; showing dashes",
                self.name, self.spec
            ),
        }
    }
}

/// For each chip matching `spec`, in libsensors order, the `_input` subfeature of
/// its first temperature whose label matches.
fn find_matches(sensors: &Sensors, spec: &SensorSpec) -> Vec<Match> {
    let mut matches = Vec::new();
    for chip in sensors.into_iter() {
        let chip_name = chip.get_name().unwrap_or_default();
        if !chip_name.to_lowercase().starts_with(&spec.device) {
            continue;
        }
        if spec.pinned_to_pci_id() && !matches_pci_id(&chip, &spec.vendor_id, &spec.device_id) {
            continue;
        }
        for feature in chip {
            if *feature.feature_type() != FeatureType::SENSORS_FEATURE_TEMP {
                continue;
            }
            let Ok(label) = feature.get_label() else {
                continue;
            };
            if !label.to_lowercase().starts_with(&spec.temp_type) {
                continue;
            }
            if let Some(input) = feature.into_iter().find(|sf| sf.name().ends_with("_input")) {
                matches.push(Match {
                    input,
                    description: format!("{chip_name} {label}"),
                });
                break;
            }
        }
    }
    matches
}

/// Whether the PCI device behind a libsensors chip has the given vendor and device
/// ID. An empty ID matches anything.
fn matches_pci_id(chip: &Chip, vendor_id: &str, device_id: &str) -> bool {
    let (Some(vendor), Some(device)) = pci_id(chip) else {
        return false;
    };
    (vendor_id.is_empty() || vendor == vendor_id) && (device_id.is_empty() || device == device_id)
}

/// The PCI vendor and device ID of the device behind a chip's hwmon directory,
/// normalised like the config values. `<hwmon>/device` is the PCI function for
/// most drivers; for class devices such as NVMe it is one level further up.
fn pci_id(chip: &Chip) -> (Option<String>, Option<String>) {
    let device = chip.path().join("device");
    for dir in [device.clone(), device.join("device")] {
        let read = |file: &str| {
            fs::read_to_string(dir.join(file))
                .ok()
                .map(|s| normalize_hex_id(&s.to_lowercase()).to_owned())
        };
        if let (Some(vendor), Some(device)) = (read("vendor"), read("device")) {
            return (Some(vendor), Some(device));
        }
    }
    (None, None)
}

fn list_sensors() {
    for chip in Sensors::new().into_iter() {
        let name = chip.get_name().unwrap_or_else(|_| "?".into());
        let id = match pci_id(&chip) {
            (Some(vendor), Some(device)) => format!("  [{vendor}:{device}]"),
            _ => String::new(),
        };
        println!("{name}{id}  ({})", chip.path().display());
        for feature in chip {
            if *feature.feature_type() != FeatureType::SENSORS_FEATURE_TEMP {
                continue;
            }
            let label = feature
                .get_label()
                .unwrap_or_else(|_| feature.name().to_owned());
            let value = feature
                .into_iter()
                .find(|sf| sf.name().ends_with("_input"))
                .and_then(|sf| sf.get_value().ok());
            match value {
                Some(v) => println!("    {label}: {v:.1} °C"),
                None => println!("    {label}: unreadable"),
            }
        }
    }
}

fn run() -> Result<()> {
    let config = match Config::load() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("error: {err:#}");
            if !Path::new(CONFIG_PATH).exists() {
                eprintln!(
                    "Create it from this template, using names from\n\
                     `antec-flux-pro-display --list-sensors` or `sensors`:\n\n{SAMPLE_CONFIG}\n"
                );
            }
            std::process::exit(EXIT_CONFIG);
        }
    };

    println!(
        "antec-flux-pro-display {VERSION}: CPU {}, GPU {}, every {} ms",
        config.cpu,
        config.gpu,
        config.update_interval.as_millis()
    );

    let display = UsbDevice::open()?;
    let sensors = Sensors::new();
    let mut cpu = Slot::new("CPU", config.cpu.clone());
    let mut gpu = Slot::new("GPU", config.gpu.clone());
    cpu.bind(&sensors);
    gpu.bind(&sensors);
    if cpu.bound.is_none() && gpu.bound.is_none() {
        bail!("neither sensor was found; check the configuration against --list-sensors");
    }

    loop {
        let started = Instant::now();
        display.send(cpu.read(), gpu.read())?;
        thread::sleep(config.update_interval);

        if started.elapsed() > config.update_interval + SUSPEND_SLACK {
            cpu.bind(&sensors);
            gpu.bind(&sensors);
            if let Err(err) = display.claim() {
                eprintln!("{err:#}");
            }
        }
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => run(),
        ["--list-sensors"] => {
            list_sensors();
            Ok(())
        }
        ["--version" | "-V"] => {
            println!("antec-flux-pro-display {VERSION}");
            Ok(())
        }
        ["--help" | "-h"] => {
            println!("{USAGE}");
            Ok(())
        }
        other => {
            eprintln!("unexpected arguments: {}\n{USAGE}", other.join(" "));
            std::process::exit(EXIT_USAGE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_ids_are_normalised() {
        assert_eq!(normalize_hex_id("0x1002"), "1002");
        assert_eq!(normalize_hex_id("1002"), "1002");
        assert_eq!(normalize_hex_id(" 0522 "), "522");
        assert_eq!(normalize_hex_id("0000"), "");
    }

    #[test]
    fn config_parses_upstream_format() {
        let config = Config::parse(
            "# comment\ncpu_device=K10Temp\ncpu_temp_type = Tctl\n\ngpu_device=amdgpu\n\
             gpu_temp_type=edge\ngpu_vendor_id=0X1002\ngpu_device_id=7550\n",
        )
        .unwrap();
        assert_eq!(config.cpu.device, "k10temp");
        assert_eq!(config.cpu.temp_type, "tctl");
        assert!(!config.cpu.pinned_to_pci_id());
        assert_eq!(config.gpu.vendor_id, "1002");
        assert_eq!(config.gpu.device_id, "7550");
        assert_eq!(config.update_interval, DEFAULT_UPDATE_INTERVAL);
    }

    #[test]
    fn config_requires_both_sensors() {
        let err = Config::parse("cpu_device=k10temp\ncpu_temp_type=tctl\n").unwrap_err();
        assert!(err.to_string().contains("gpu_device"));
    }

    #[test]
    fn update_interval_is_validated() {
        let base = "cpu_device=a\ncpu_temp_type=b\ngpu_device=c\ngpu_temp_type=d\n";
        assert!(Config::parse(&format!("{base}update_interval=abc\n")).is_err());
        let fast = Config::parse(&format!("{base}update_interval=50\n")).unwrap();
        assert_eq!(fast.update_interval, MIN_UPDATE_INTERVAL);
        let slow = Config::parse(&format!("{base}update_interval=2500\n")).unwrap();
        assert_eq!(slow.update_interval, Duration::from_millis(2500));
    }
}
