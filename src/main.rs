mod usb;

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::io;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sensors::{Chip, Subfeature, Sensors};
use usb::UsbDevice;

#[derive(Debug)]
struct AppConfig {
    cpu_device: String,
    cpu_temp_type: String,
    cpu_vendor_id: String,
    cpu_device_id: String,
    gpu_device: String,
    gpu_temp_type: String,
    gpu_vendor_id: String,
    gpu_device_id: String,
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

        // Required CPU values
        let cpu_device = config_map.get("cpu_device")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "cpu_device not found in config"))?
            .clone()
            .to_lowercase();

        let cpu_temp_type = config_map.get("cpu_temp_type")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "cpu_temp_type not found in config"))?
            .clone()
            .to_lowercase();

        // Optional CPU values (default to empty string if not found)
        let cpu_vendor_id = config_map.get("cpu_vendor_id").cloned().unwrap_or_default().trim_start_matches("0x").trim().to_lowercase();
        let cpu_device_id = config_map.get("cpu_device_id").cloned().unwrap_or_default().trim_start_matches("0x").trim().to_lowercase();

        // Required GPU values
        let gpu_device = config_map.get("gpu_device")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "gpu_device not found in config"))?
            .clone()
            .to_lowercase();

        let gpu_temp_type = config_map.get("gpu_temp_type")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "gpu_temp_type not found in config"))?
            .clone()
            .to_lowercase();

        // Optional GPU values (default to empty string if not found)
        let gpu_vendor_id = config_map.get("gpu_vendor_id").cloned().unwrap_or_default().trim_start_matches("0x").trim().to_lowercase();
        let gpu_device_id = config_map.get("gpu_device_id").cloned().unwrap_or_default().trim_start_matches("0x").trim().to_lowercase();

        // Update interval is optional, default to 1000ms if not found
        let update_interval = config_map.get("update_interval")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000);

        Ok(AppConfig {
            cpu_device,
            cpu_temp_type,
            cpu_vendor_id,
            cpu_device_id,
            gpu_device,
            gpu_temp_type,
            gpu_vendor_id,
            gpu_device_id,
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

// Helper function to find sensors
fn discover_features(sensors: &Sensors, config: &AppConfig) -> (Option<Subfeature>, Option<Subfeature>) {
    let mut cpu: Option<Subfeature> = None;
    let mut gpu: Option<Subfeature> = None;

    for chip in sensors.clone().into_iter() {
        let chip_name = chip.get_name().unwrap_or_default().to_lowercase();

        // CPU PCI Vendor/Device check
        let cpu_v_empty = config.cpu_vendor_id.is_empty();
        let cpu_d_empty = config.cpu_device_id.is_empty();

        let is_cpu_pci = if cpu_v_empty && cpu_d_empty {
            true // Default to true if both are empty
        } else {
            matches_pci_id(&chip, &config.cpu_vendor_id, &config.cpu_device_id)
        };

        // GPU PCI Vendor/Device check
        let gpu_v_empty = config.gpu_vendor_id.is_empty();
        let gpu_d_empty = config.gpu_device_id.is_empty();

        let is_gpu_pci = if gpu_v_empty && gpu_d_empty {
            true // Default to true if both are empty
        } else {
            matches_pci_id(&chip, &config.gpu_vendor_id, &config.gpu_device_id)
        };

        for feature in chip {
            if let Ok(label) = feature.get_label() {
                let label_lower = label.to_lowercase();

                let is_cpu = (chip_name.starts_with(&config.cpu_device) || label_lower.starts_with(&config.cpu_device))
                    && label_lower.starts_with(&config.cpu_temp_type)
                    && is_cpu_pci;

                let is_gpu = (chip_name.starts_with(&config.gpu_device) || label_lower.starts_with(&config.gpu_device))
                    && label_lower.starts_with(&config.gpu_temp_type)
                    && is_gpu_pci;

                if is_cpu || is_gpu {
                    for subfeature in feature {
                        if subfeature.name().contains("input") {
                            if is_cpu {
                                cpu = Some(subfeature);
                            } else if is_gpu {
                                gpu = Some(subfeature);
                            }
                        }
                    }
                }
            }
        }
    }

    if cpu.is_none() {
        if config.cpu_device.is_empty() {
            eprintln!(
                "Error: CPU device matching '{}' with type '{}' not found!",
                config.cpu_device, config.cpu_temp_type
            );
        }
        else {
            eprintln!(
                "Error: CPU device matching '{}', vendor_id '{}', device_id '{}' with type '{}' not found!",
                config.cpu_device, config.cpu_vendor_id, config.cpu_device_id, config.cpu_temp_type
            );
        }
    }
    if gpu.is_none() {
        if config.gpu_device.is_empty() {
            eprintln!(
                "Error: GPU device matching '{}' with type '{}' not found!",
                config.gpu_device, config.gpu_temp_type
            );
        }
        else {
            eprintln!(
                "Error: GPU device matching '{}', vendor_id '{}', device_id '{}' with type '{}' not found!",
                config.gpu_device, config.gpu_vendor_id, config.gpu_device_id, config.gpu_temp_type
            );
        }
    }

    (cpu, gpu)
}

fn matches_pci_id(chip: &Chip, expected_vendor: &str, expected_device: &str) -> bool {
    let chip_name = chip.get_name().unwrap_or_default();

    // Extract the hex part from names like "amdgpu-pci-0300"
    // The last part '0300' represents Bus (03) and Device/Function (00)
    let hex_addr = match chip_name.split('-').last() {
        Some(h) if h.len() >= 4 => h,
        _ => return false,
    };

    // Iterate through /sys/bus/pci/devices to find a match
    if let Ok(entries) = fs::read_dir("/sys/bus/pci/devices/") {
        for entry in entries.flatten() {
            let pci_id = entry.file_name().into_string().unwrap_or_default();

            // Convert DBDF (0000:03:00.0) to libsensors format (0300)
            // 0000 : [Bus] : [Device].[Function]
            let parts: Vec<&str> = pci_id.split(|c| c == ':' || c == '.').collect();
            if parts.len() >= 4 {
                let bus = parts[1]; // "03"
                let dev = parts[2]; // "00"
                let func = parts[3]; // "0"

                // libsensors "0300" = Bus(03) + Device(00) + Function(0)
                let reconstructed = format!("{}{}{}", bus, dev, func);

                // Compare against the chip's internal address (e.g., "0300")
                if reconstructed.starts_with(hex_addr) {
                    let path = entry.path();
                    let vendor = fs::read_to_string(path.join("vendor")).unwrap_or_default();
                    let device = fs::read_to_string(path.join("device")).unwrap_or_default();

                    let clean_v = expected_vendor.to_lowercase();
                    let clean_d = expected_device.to_lowercase();

                    if vendor.trim().contains(&clean_v) && device.trim().contains(&clean_d) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize sensors first
    let mut sensors = Sensors::new();

    // Print available sensors
    println!("Available temperature sensors:");
    println!("-----------------------------");

    // List all available chips and their features
    for chip in sensors {
        println!("Chip: {}", chip.get_name()?);

        let hwmon_path = chip.path();
        println!("  Path: {}", hwmon_path.display());

        // Get the device path (e.g., /sys/class/hwmon/hwmon1/device)
        let device_path = hwmon_path.join("device");

        // Read vendor and device IDs (stored as hex strings like "0x1002\n")
        let vendor_hex = fs::read_to_string(device_path.join("vendor")).unwrap_or_default();
        let device_hex = fs::read_to_string(device_path.join("device")).unwrap_or_default();
        let vendor_trimmed = vendor_hex.trim_start_matches("0x").trim();
        let device_trimmed = device_hex.trim_start_matches("0x").trim();
        println!("  VendorId: {}", vendor_trimmed);
        println!("  DeviceId: {}", device_trimmed);
        println!("  Temperatures:");
        for feature in chip {
            if let Some(label) = feature.get_label().ok() {
                if *feature.feature_type() == sensors::FeatureType::SENSORS_FEATURE_TEMP {
                    if let Some(input) = feature.into_iter().find(|sf| sf.name().contains("input")) {
                        if let Ok(temp) = input.get_value() {
                            println!("    {}: {:.1}°C", label, temp);
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

    // Need to claim the interface to continue.
    device.claim_interface();

    let (mut cpu_feature, mut gpu_feature) = discover_features(&sensors, &config);

    if cpu_feature.is_none() && gpu_feature.is_none() {
        println!("Both CPU and GPU devices were not found. Please check your config or run 'sensors' in terminal to see available names. Program exiting.");
        std::process::exit(1);
    }

    loop {
        let start_time = Instant::now();

        // Attempt to read temperatures
        let cpu_temp = cpu_feature.as_ref().and_then(|f| f.get_value().ok());
        let gpu_temp = gpu_feature.as_ref().and_then(|f| f.get_value().ok());

        // Handle missing sensors (e.g., driver unloaded/reloaded during sleep)
        if cpu_temp.is_none() && gpu_temp.is_none() {
            #[cfg(debug_assertions)]
            eprintln!("Sensors lost. Attempting re-discovery...");

            sensors = Sensors::new(); // Refresh libsensors internal state
            let (c, g) = discover_features(&sensors, &config);
            cpu_feature = c;
            gpu_feature = g;
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

        // Sleep and Detect Wake-up
        let interval = Duration::from_millis(config.update_interval);
        thread::sleep(interval);

        // If we slept for much longer than intended, assume the PC was suspended
        if start_time.elapsed() > interval + Duration::from_secs(2) {
            #[cfg(debug_assertions)]
            println!("Wake-up detected. Refreshing hardware handles...");

            sensors = Sensors::new();
            let (c, g) = discover_features(&sensors, &config);
            cpu_feature = c;
            gpu_feature = g;

            device.claim_interface();
        }
    }
}
