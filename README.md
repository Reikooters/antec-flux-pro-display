# Antec Flux Pro Display

![Release](https://img.shields.io/github/v/release/Reikooters/antec-flux-pro-display)
![License](https://img.shields.io/github/license/Reikooters/antec-flux-pro-display)

A Linux application which will output your CPU and GPU temperature onto the [Antec Flux Pro](https://www.antec.com/product/case/flux-pro) case display.

It uses the `sensors` rust crate, which uses `libsensors`, so works with any CPU or GPU, but you will need to do some manual installation steps described below.

This project builds upon [nishtahir/antec-flux-pro-display](https://github.com/nishtahir/antec-flux-pro-display) and [AKoskovich/antec_flux_pro_display_service](https://github.com/AKoskovich/antec_flux_pro_display_service). While maintaining the performance benefits of Rust from the first project, it improves temperature monitoring by allowing custom configuration of device and sensor names, rather than assuming fixed hardware monitor assignments. This ensures accurate temperature readings across different system configurations.

## Table of Contents

- [Features](#features)
- [System Requirements](#system-requirements)
- [Installation Instructions](#installation-instructions)
- [Troubleshooting](#troubleshooting)
- [Configuration](#configuration)
- [Uninstalling](#uninstalling)
- [Development](#development)
- [Contributing](#contributing)
- [Credits](#credits)
- [License](#license)

## Features

- Real-time CPU and GPU temperature monitoring
- Configurable sensor sources for maximum compatibility
- Systemd service for automatic startup
- Low resource usage through Rust implementation
- Easy configuration through a simple config file

## System Requirements

### Hardware

- Antec Flux Pro case with built-in display
- Case display plugged into USB port on motherboard

### Software

- Linux distribution with systemd (tested on Kernel 6.14.9)
- Required packages:

  | Distribution | Package Names |
  |-------------|---------------|
  | Debian/Ubuntu | `lm-sensors`, `usbutils` |
  | Arch Linux | `lm_sensors`, `usbutils` |
  | Fedora | `lm_sensors`, `usbutils` |

> [!TIP]
> The `usbutils` package isn't technically required for the application to function, it's just used during the installation steps to give you the `lsusb` command, which is used to check to ensure the case display is correctly plugged into your motherboard and recognised.

### Dependencies

- `libsensors` (provided by `lm-sensors` package)
- Proper USB permissions (configured during installation)

## Installation Instructions

### 1. Set up permission to write to the display

1. Ensure the Antec Flux Pro case display is connected to your computer:

```shell
lsusb | grep "2022:0522"
```

Output should look something like the below. If there is no output, then the case display isn't connected or recognised.
```
Bus 001 Device 005: ID 2022:0522 Љ Љ
```

> [!NOTE]
> If you don't have the `lsusb` command available on your computer, install `usbutils` using your package manager, for example on Arch Linux use `sudo pacman -S usbutils` or on Debian/Ubuntu use `sudo apt install usbutils`.

2. Create a udev rules file to allow yourself permission to write to the display:

```shell
sudo nano /etc/udev/rules.d/99-antec-flux-pro-display.rules
```

In this file, copy and paste the following content:

```
SUBSYSTEM=="usb", ATTR{idVendor}=="2022", ATTR{idProduct}=="0522", MODE="0660", TAG+="uaccess"
```

Press Ctrl+X to quit Nano, pressing Y to say Yes to saving the file, and press Enter when prompted for the path to write to.

3. You must **reboot your computer** for the permission change to take effect.

### 2. Configure sensors

1. Identify the names of the sensors for your CPU and GPU. To do this, first run the following command:

```shell
sensors
```

Output should look something like the below.

```
k10temp-pci-00c3
Adapter: PCI adapter
Tctl:         +39.1°C  

nvme-pci-0400
Adapter: PCI adapter
Composite:    +30.9°C  (low  =  -0.1°C, high = +85.8°C)
                       (crit = +87.8°C)
Sensor 1:     +30.9°C  (low  = -273.1°C, high = +65261.8°C)

r8169_0_1000:00-mdio-0
Adapter: MDIO adapter
temp1:        +37.0°C  (high = +120.0°C)

amdgpu-pci-0300
Adapter: PCI adapter
vddgfx:      228.00 mV 
fan1:           0 RPM  (min =    0 RPM, max = 3000 RPM)
edge:         +28.0°C  (crit = +110.0°C, hyst = -273.1°C)
                       (emerg = +115.0°C)
junction:     +30.0°C  (crit = +110.0°C, hyst = -273.1°C)
                       (emerg = +115.0°C)
mem:          +50.0°C  (crit = +108.0°C, hyst = -273.1°C)
                       (emerg = +113.0°C)
PPT:          15.00 W  (cap = 340.00 W)
pwm1:              0%
sclk:          24 MHz 
mclk:         456 MHz 
```

Take note of the device name (the part before `-pci` in each heading) as well as the sensor label (to the left of the temperature).

For my computer, my CPU device name is `k10temp` and sensor name is `Tctl`. For my GPU, the device name is `amdgpu` and sensor name is `edge`.

2. Create a configuration file to be used by the application.

```shell
sudo nano /etc/antec-flux-pro-display/config.conf
```

In this file, copy and paste the following content, substituting your device and sensor names as appropriate. Device and sensor names are not case sensitive and can be specified as lowercase.

```conf
# CPU device for temperature monitoring
cpu_device=k10temp
cpu_temp_type=tctl

# GPU device for temperature monitoring
gpu_device=amdgpu
gpu_temp_type=edge

# Update interval in milliseconds
update_interval=1000
```

Press Ctrl+X to quit Nano, pressing Y to say Yes to saving the file, and press Enter when prompted for the path to write to.

> [!NOTE]
> #### Note if you have two devices with the same name:
> 
> If you have two devices with the same name, for example if you have both an AMD GPU and an AMD CPU with integrated graphics enabled, you might have two devices both named `amdgpu`. This means you'll need to also add the device's VendorId and DeviceId to the configuration in order to more explicitly specify the device.
>
> Run the following in your teminal:
>
> ```sh
> for d in /sys/class/hwmon/hwmon*; do
>     # Display the basic device info
>     echo -n "$(basename $d): $(cat $d/name) | "
>     [ -f $d/device/vendor ] && echo -n "Vendor: $(cat $d/device/vendor) Device: $(cat $d/device/device) " || echo -n "Virtual Device "
>     echo "---"
> 
>     # Display each temperature sensor found in this hwmon directory
>     for t in $d/temp*_input; do
>         if [ -f "$t" ]; then
>             # Get the label (e.g., 'edge') or use 'tempX' if no label exists
>             label_file="${t%_input}_label"
>             if [ -f "$label_file" ]; then
>                 label=$(cat "$label_file")
>             else
>                 label=$(basename "${t%_input}")
>             fi
>             
>             # Read the temperature (stored in millidegrees Celsius)
>             temp_raw=$(cat "$t")
>             temp_c=$((temp_raw / 1000))
> 
>             echo "  └─ $label: ${temp_c}°C"
>         fi
>     done
> done
> ```
>
> Output should be similar to the below.
>
> ```
> hwmon0: nvme | Virtual Device ---
>   └─ Composite: 42°C
>   └─ Sensor 1: 42°C
> hwmon1: amdgpu | Vendor: 0x1002 Device: 0x7550 ---
>   └─ edge: 39°C
>   └─ junction: 41°C
>   └─ mem: 58°C
> hwmon2: k10temp | Vendor: 0x1022 Device: 0x14e3 ---
>   └─ Tctl: 58°C
>   └─ Tccd1: 48°C
>   └─ Tccd2: 66°C
> hwmon3: r8169_0_1000:00 | Virtual Device ---
>   └─ temp1: 48°C
> ```
>
> Set up your configuration as follows, adding the `gpu_vendor_id` and `gpu_device_id` configuration options, shown below. (The program will accept the configuration with or without the `0x`)
>
> ```conf
> # CPU device for temperature monitoring
> cpu_device=k10temp
> cpu_temp_type=tctl
> cpu_vendor_id=1022
> cpu_device_id=14e3
> 
> # GPU device for temperature monitoring
> gpu_device=amdgpu
> gpu_temp_type=edge
> gpu_vendor_id=1002
> gpu_device_id=7550
> 
> # Update interval in milliseconds
> update_interval=1000
> ```
> As shown above, there are also configuration options available for `cpu_vendor_id` and `cpu_device_id` - most people won't need these. They are provided in case you want to show some other device with a conflicting name in the CPU position on the case display, such as a second GPU.
>
> #### If you need to know more details about a specific device (which is which), there is a round about way to do this:
>
> 1. Get the bus ID for the device using the following command, where `hwmon1` comes from the output of the command above where you got the vendor ID and device ID. If yours is something else, such as `hwmon2`, then replace it in the command below.
>
> ```sh
> ls -l /sys/class/hwmon/hwmon1/device
> ```
>
> Output should be similar to this:
>
> ```
> lrwxrwxrwx 1 root root 0 Dec 15 21:31 /sys/class/hwmon/hwmon1/device -> ../../../0000:03:00.0
> ```
>
> 2. Take the bus ID (string at the end of the output above) and use the `lspci` command as follows:
>
> ```sh
> lspci -s 0000:03:00.0
> ```
>
> This will output a more meaningful device name:
>
> ```sh
> 03:00.0 VGA compatible controller: Advanced Micro Devices, Inc. [AMD/ATI] Navi 48 [Radeon RX 9070/9070 XT/9070 GRE] (rev c0)
> ```

### 3. Download the application and install the service

1. Download the `antec-flux-pro-display` binary from [Releases](https://github.com/Reikooters/antec-flux-pro-display/releases). Then use `install` to copy it to `/usr/bin/antec-flux-pro-display` and make the file executable. Example:

```shell
curl -L -o antec-flux-pro-display "https://github.com/Reikooters/antec-flux-pro-display/releases/download/v1.2/antec-flux-pro-display"
sudo install antec-flux-pro-display /usr/bin/
```

2. Create a service file for the application:

```shell
sudo nano /etc/systemd/system/antec-flux-pro-display.service
```

In this file, copy and paste the following content:

```
[Unit]
Description=Antec Flux Pro Display Service
StartLimitIntervalSec=0

[Service]
Type=simple
ExecStart=/usr/bin/antec-flux-pro-display
Restart=always
RestartSec=5
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
```

Press Ctrl+X to quit Nano, pressing Y to say Yes to saving the file, and press Enter when prompted for the path to write to.

3. Start the service and enable it to run at boot:

```
sudo systemctl daemon-reload
sudo systemctl start antec-flux-pro-display
sudo systemctl enable antec-flux-pro-display
```

The display should now be working.

## Troubleshooting

### Display Not Updating

- Check if the service is running: `systemctl status antec-flux-pro-display`
- Check to ensure you created the file for the udev rule correctly (as per instructions). Remember that **a reboot is required** in order for the change to take effect.
- Check sensor availability: `sensors`. If your computer doesn't have this command avaiable, installed the `lm-sensors` package from your distribution's package manager.

### Wrong Temperature Readings

- Verify sensor names in config match output of the `sensors` command
- Ensure the correct sensor label is used (e.g., `Tctl` for AMD CPUs)
- See the notes in installation steps about specifying the VendorId and DeviceId if you have two devices with the same name, e.g. two `amdgpu`.

## Configuration

The configuration file `/etc/antec-flux-pro-display/config.conf` supports the following options:

| Option | Description | Example |
|--------|-------------|---------|
| cpu_device | CPU temperature device name | `k10temp` |
| cpu_temp_type | CPU temperature sensor label | `tctl` |
| cpu_vendor_id | **Optional**, use it in addition to the name if you have two devices with the same name | `1022` |
| cpu_device_id | **Optional**, use it in addition to the name if you have two devices with the same name | `14e3` |
| gpu_device | GPU temperature device name | `amdgpu` |
| gpu_temp_type | GPU temperature sensor label | `edge` |
| gpu_vendor_id | **Optional**, use it in addition to the name if you have two devices with the same name | `1002` |
| gpu_device_id | **Optional**, use it in addition to the name if you have two devices with the same name | `7550` |
| update_interval | Update frequency in milliseconds | `1000` |

### Service Won't Start

- Check logs: `journalctl -u antec-flux-pro-display -n 50 --no-pager`
- Verify config file syntax
- Ensure USB device is connected (as per instructions)
- Ensure you have **rebooted your computer** if you only just created the udev rules file.

## Uninstalling

To uninstall, stop and disable the service, then remove the files which you created during the installation steps.

```shell
sudo systemctl stop antec-flux-pro-display
sudo systemctl disable antec-flux-pro-display
sudo rm /etc/udev/rules.d/99-antec-flux-pro-display.rules
sudo rm /etc/antec-flux-pro-display/config.conf
sudo rm /etc/systemd/system/antec-flux-pro-display.service
sudo rm /usr/bin/antec-flux-pro-display
sudo systemctl daemon-reload
```

## Development

### Building from Source

```shell
git clone https://github.com/Reikooters/antec-flux-pro-display
cd antec-flux-pro-display
cargo build --release
```

### Dependencies

- Rust 1.87.0 or later
- Libraries: anyhow 1.0.100, rusb 0.9.4, sensors 0.2.2

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

## Credits

### Original Projects

- [nishtahir/antec-flux-pro-display](https://github.com/nishtahir/antec-flux-pro-display) - Original Rust implementation
- [AKoskovich/antec_flux_pro_display_service](https://github.com/AKoskovich/antec_flux_pro_display_service) - Original concept

### Code Attribution

- The `usb.rs` module is adapted from nishtahir's project ([source](https://github.com/nishtahir/antec-flux-pro-display/blob/main/src/usb.rs)), with modifications to use `f64` for temperature variables

### Contributors

- [@LostSyndicate](https://github.com/LostSyndicate)
  - Improved USB compatibility by implementing device claiming to prevent OS permission conflicts.
  - Refactored `send_payload` to use `write_interrupt` to correctly match the USB endpoint transfer type.

### Development Tools

- Developed using RustRover IDE with JetBrains AI Assistant plugin
- AI assistance provided by Claude 3.5 Sonnet

## License

This project is licensed under [GNU GENERAL PUBLIC LICENSE Version 3](LICENSE) - see the LICENSE file for details.
