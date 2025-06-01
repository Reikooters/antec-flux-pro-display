mod usb;

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::io;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sensors::Sensors;
use usb::UsbDevice;

#[derive(Debug)]
struct AppConfig {
    cpu_device: String,
    gpu_device: String,
    cpu_temp_type: String,
    gpu_temp_type: String,
    update_interval: u64,
}

impl AppConfig {
    fn new() -> io::Result<Self> {
        let config_dir = PathBuf::from("/etc/antec-flux-pro-display");
        let config_path = config_dir.join("config.conf");

        if !config_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Configuration file /etc/antec-flux-pro-display/config.conf not found"
            ));
        }

        let config_str = fs::read_to_string(config_path)?;
        let mut config_map = HashMap::new();

        for line in config_str.lines() {
            // Skip empty lines and comments
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                config_map.insert(key.trim().to_string(), value.trim().to_string());
            }
        }

        // Get required values or return error if not found
        let cpu_device = config_map.get("cpu_device")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "cpu_device not found in config"))?
            .clone();

        let gpu_device = config_map.get("gpu_device")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "gpu_device not found in config"))?
            .clone();

        let cpu_temp_type = config_map.get("cpu_temp_type")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "cpu_temp_type not found in config"))?
            .clone();

        let gpu_temp_type = config_map.get("gpu_temp_type")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "gpu_temp_type not found in config"))?
            .clone();

        // Update interval is optional, default to 1000ms if not found
        let update_interval = config_map.get("update_interval")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000);

        Ok(AppConfig {
            cpu_device,
            gpu_device,
            cpu_temp_type,
            gpu_temp_type,
            update_interval,
        })
    }
}

fn get_time_string() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    format!("{:02}:{:02}:{:02}",
            (secs / 3600) % 24,
            (secs / 60) % 60,
            secs % 60)
}

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize sensors first
    let sensors = Sensors::new();

    // Print available sensors
    println!("Available temperature sensors:");
    println!("-----------------------------");

    // List all available chips and their features
    for chip in sensors {
        println!("Chip: {}", chip.get_name()?);
        for feature in chip {
            if let Some(label) = feature.get_label().ok() {
                if *feature.feature_type() == sensors::FeatureType::SENSORS_FEATURE_TEMP {
                    if let Some(input) = feature.into_iter().find(|sf| sf.name().contains("input")) {
                        if let Ok(temp) = input.get_value() {
                            println!("  {}: {:.1}°C", label, temp);
                        }
                    }
                }
            }
        }
    }

    println!("-----------------------------\n");

    // Load configuration
    let config = match AppConfig::new() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);

            if !Path::new("/etc/antec-flux-pro-display/config.conf").exists() {
                // Sample configuration template
                const SAMPLE_CONFIG: &str = r#"# CPU device for temperature monitoring
cpu_device=k10temp
cpu_temp_type=tctl

# GPU device for temperature monitoring
gpu_device=amdgpu
gpu_temp_type=edge

# Update interval in milliseconds
update_interval=1000"#;

                eprintln!("\nConfiguration file is missing. Please create the following file:");
                eprintln!("/etc/antec-flux-pro-display/config.conf");
                eprintln!("\nUUse this sample configuration as a template and adjust according to your available sensors above:");
                eprintln!("-----------------------------");
                eprintln!("{}", SAMPLE_CONFIG);
                eprintln!("-----------------------------");
                eprintln!("\nMake sure to adjust the values according to your system's available sensors shown above.");
            }

            std::process::exit(1);
        }
    };

    // Pre-compute lowercase versions of search patterns
    let cpu_device_lower = config.cpu_device.to_lowercase();
    let gpu_device_lower = config.gpu_device.to_lowercase();
    let cpu_temp_type_lower = config.cpu_temp_type.to_lowercase();
    let gpu_temp_type_lower = config.gpu_temp_type.to_lowercase();

    // Print initial information
    println!("Starting temperature monitor...");
    println!("CPU device: {} (type: {})", config.cpu_device, config.cpu_temp_type);
    println!("GPU device: {} (type: {})", config.gpu_device, config.gpu_temp_type);
    println!("Update interval: {}ms\n", config.update_interval);

    let device = UsbDevice::open(usb::VENDOR_ID, usb::PRODUCT_ID)?;

    loop {
        let mut cpu_temp: Option<f64> = None;
        let mut gpu_temp: Option<f64> = None;

        for chip in sensors.clone() {
            let chip_name = chip.get_name()?.to_lowercase();

            for feature in chip {
                if let Ok(label) = feature.get_label() {
                    let label = label.to_lowercase();

                    let is_cpu_device = chip_name.contains(&cpu_device_lower) ||
                        label.contains(&cpu_device_lower);
                    let is_gpu_device = chip_name.contains(&gpu_device_lower) ||
                        label.contains(&gpu_device_lower);

                    if is_cpu_device && label.contains(&cpu_temp_type_lower) {
                        if let Some(input) = feature.into_iter().find(|sf| sf.name().contains("input")) {
                            if let Ok(temp) = input.get_value() {
                                cpu_temp = Some(temp);
                            }
                        }
                    } else if is_gpu_device && label.contains(&gpu_temp_type_lower) {
                        if let Some(input) = feature.into_iter().find(|sf| sf.name().contains("input")) {
                            if let Ok(temp) = input.get_value() {
                                gpu_temp = Some(temp);
                            }
                        }
                    }
                }
            }
        }

        match (cpu_temp, gpu_temp) {
            (Some(_cpu), Some(_gpu)) => {
                #[cfg(debug_assertions)]
                println!("[{}] CPU Temperature: {:.1}°C  |  GPU Temperature: {:.1}°C",
                         get_time_string(), _cpu, _gpu);
            },
            (Some(_cpu), None) => {
                #[cfg(debug_assertions)]
                println!("[{}] CPU Temperature: {:.1}°C  |  GPU device '{}', temp type '{}' not found!",
                         get_time_string(), _cpu, gpu_device_lower, gpu_temp_type_lower);
                #[cfg(not(debug_assertions))]
                eprintln!("[{}] GPU device '{}', temp type '{}' not found!",
                          get_time_string(), gpu_device_lower, gpu_temp_type_lower);
            },
            (None, Some(_gpu)) => {
                #[cfg(debug_assertions)]
                println!("[{}] CPU device '{}', temp type '{}' not found!  |  GPU Temperature: {:.1}°C",
                         get_time_string(), cpu_device_lower, cpu_temp_type_lower, _gpu);
                #[cfg(not(debug_assertions))]
                eprintln!("[{}] CPU device '{}', temp type '{}' not found!",
                          get_time_string(), cpu_device_lower, cpu_temp_type_lower);
            },
            (None, None) => {
                eprintln!("[{}] CPU device '{}', temp type '{}' not found!  |  GPU device '{}', temp type '{}' not found!",
                          get_time_string(), cpu_device_lower, cpu_temp_type_lower, gpu_device_lower, gpu_temp_type_lower);
            }
        }

        device.send_payload(&cpu_temp, &gpu_temp);
        thread::sleep(Duration::from_millis(config.update_interval));
    }
}
