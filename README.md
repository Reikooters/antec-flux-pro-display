# Antec Flux Pro Display

![Release](https://img.shields.io/github/v/release/Reikooters/antec-flux-pro-display)
![License](https://img.shields.io/github/license/Reikooters/antec-flux-pro-display)

A Linux application which will output your CPU and GPU temperature onto the [Antec Flux Pro](https://www.antec.com/product/case/flux-pro) case display.

It uses the `sensors` rust crate, which uses `libsensors`, so works with any CPU or GPU, but you will need to do some manual installation steps described below.

This project builds upon [nishtahir/antec-flux-pro-display](https://github.com/nishtahir/antec-flux-pro-display) and [AKoskovich/antec_flux_pro_display_service](https://github.com/AKoskovich/antec_flux_pro_display_service). While maintaining the performance benefits of Rust from the first project, it improves temperature monitoring by allowing custom configuration of device and sensor names, rather than assuming fixed hardware monitor assignments. This ensures accurate temperature readings across different system configurations.

## Table of Contents

- [Quick Start](#quick-start)
- [System Requirements](#system-requirements)
- [Features](#features)
- [Development](#development)
- [Contributing](#contributing)
- [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)
- [Detailed Installation Instructions](#detailed-installation-instructions)
- [Uninstalling](#uninstalling)
- [Credits](#credits)
- [License](#license)

## Quick Start

Quick setup for advanced users. See [Detailed Installation Instructions](#detailed-installation-instructions) section for more details.

### 1. Set up USB permissions

```shell
sudo bash -c 'cat > /etc/udev/rules.d/99-antec-flux-pro-display.rules << EOL
SUBSYSTEM=="usb", ATTR{idVendor}=="2022", ATTR{idProduct}=="0522", MODE="0666", GROUP="plugdev", TAG+="uaccess"
EOL'
sudo udevadm control --reload-rules && sudo udevadm trigger
```

### 2. Find your sensor names

```shell
sensors
```

### 3. Configure sensors

```shell
sudo mkdir -p /etc/antec-flux-pro-display/
sudo bash -c 'cat > /etc/antec-flux-pro-display/config.conf << EOL
# CPU device for temperature monitoring
cpu_device=k10temp
cpu_temp_type=tctl

# GPU device for temperature monitoring
gpu_device=amdgpu
gpu_temp_type=edge

# Update interval in milliseconds
update_interval=1000
EOL'
```

### 4. Download and install binary

Download latest release from https://github.com/Reikooters/antec-flux-pro-display/releases

```shell
curl -L -o antec-flux-pro-display "https://github.com/Reikooters/antec-flux-pro-display/releases/download/latest/antec-flux-pro-display"
sudo install antec-flux-pro-display /usr/bin/
```

### 5. Create and start the service

```shell
sudo bash -c 'cat > /etc/systemd/system/antec-flux-pro-display.service << EOL
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
EOL'

sudo systemctl daemon-reload
sudo systemctl start antec-flux-pro-display
sudo systemctl enable antec-flux-pro-display
```

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

### Dependencies

- `libsensors` (provided by `lm-sensors` package)
- Proper USB permissions (configured during installation)

## Features

- Real-time CPU and GPU temperature monitoring
- Configurable sensor sources for maximum compatibility
- Systemd service for automatic startup
- Low resource usage through Rust implementation
- Easy configuration through a simple config file

## Development

### Building from Source

```shell
git clone https://github.com/YOUR_USERNAME/antec-flux-pro-display
cd antec-flux-pro-display
cargo build --release
```

### Dependencies

- Rust 1.87.0 or later
- Libraries: anyhow 1.0.98, rusb 0.9.4, sensors 0.2.2

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

## Configuration

The configuration file (`/etc/antec-flux-pro-display/config.conf`) supports the following options:

| Option | Description | Example |
|--------|-------------|---------|
| cpu_device | CPU temperature device name | `k10temp` |
| cpu_temp_type | CPU temperature sensor label | `tctl` |
| gpu_device | GPU temperature device name | `amdgpu` |
| gpu_temp_type | GPU temperature sensor label | `edge` |
| update_interval | Update frequency in milliseconds | `1000` |

## Troubleshooting

### Display Not Updating

- Check if the service is running: `systemctl status antec-flux-pro-display`
- Verify USB permissions: `ls -l /dev/bus/usb/$(lsusb -d 2022:0522 | cut -d' ' -f2,4 | sed 's/:/\//')`
- Check sensor availability: `sensors`

### Wrong Temperature Readings

- Verify sensor names in config match output of `sensors` command
- Try different sensor labels if available (e.g., `Tdie` instead of `Tctl` for AMD CPUs)

### Service Won't Start

- Check logs: `journalctl -u antec-flux-pro-display -n 50 --no-pager`
- Verify config file syntax
- Ensure USB device is connected

## Detailed Installation Instructions

### Set up permission to write to the display

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
SUBSYSTEM=="usb", ATTR{idVendor}=="2022", ATTR{idProduct}=="0522", MODE="0666", GROUP="plugdev"
SUBSYSTEM=="usb", ATTR{idVendor}=="2022", ATTR{idProduct}=="0522", MODE="0666", GROUP="plugdev", TAG+="uaccess"
```

Press Ctrl+X to quit Nano, pressing Y to say Yes to saving the file, and press Enter when prompted for the path to write to.

> [!NOTE]
> If you are using a non-systemd Linux distribution, you'll need to also add yourself to the `plugdev` group using the below command.
>
> ```shell
> sudo usermod -a -G plugdev $USER
> ```
>
> On KDE (or any modern systemd-based desktop environment), you do NOT need to perform this step. This is because the `TAG+="uaccess"` in the second rule above automatically grants permission for the user currently logged into the local desktop session. Older systems will use the first rule, while modern systemd-based systems will use the second rule.
>
> Note: You'll need to log out and log back in for the group change to take effect.

3. Reload the udev rules:

```sh
sudo udevadm control --reload-rules
sudo udevadm trigger
```

### Configure sensors

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

Take note of the device name (the part before `-pci` in each heading) as well as the sensor label (next to the temperature).

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

### Install the service

1. Download the `antec-flux-pro-display` binary from [Releases](https://github.com/Reikooters/antec-flux-pro-display/releases). Then use `install` to copy it to `/usr/bin/antec-flux-pro-display` and make the file executable. Example:

```shell
curl -L -o antec-flux-pro-display "https://github.com/Reikooters/antec-flux-pro-display/releases/download/latest/antec-flux-pro-display"
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

3. Start the service and enable it to run at boot:

```
sudo systemctl daemon-reload
sudo systemctl start antec-flux-pro-display
sudo systemctl enable antec-flux-pro-display
```

The display should now be working.

## Uninstalling

To uninstall, stop and disable the service, then remove the files which we created during the installation steps.

```shell
sudo systemctl stop antec-flux-pro-display
sudo systemctl disable antec-flux-pro-display
sudo rm /etc/udev/rules.d/99-antec-flux-pro-display.rules
sudo rm /etc/antec-flux-pro-display/config.conf
sudo rm /etc/systemd/system/antec-flux-pro-display.service
sudo rm /usr/bin/antec-flux-pro-display
sudo systemctl daemon-reload
sudo udevadm control --reload-rules
sudo udevadm trigger
```

## Credits

### Original Projects
- [nishtahir/antec-flux-pro-display](https://github.com/nishtahir/antec-flux-pro-display) - Original Rust implementation
- [AKoskovich/antec_flux_pro_display_service](https://github.com/AKoskovich/antec_flux_pro_display_service) - Original concept

### Code Attribution
- The `usb.rs` module is adapted from nishtahir's project ([source](https://github.com/nishtahir/antec-flux-pro-display/blob/main/src/usb.rs)), with modifications to use `f64` for temperature variables

### Development Tools
- Developed using RustRover IDE with JetBrains AI Assistant plugin
- AI assistance provided by Claude 3.5 Sonnet

## License

This project is licensed under [GNU GENERAL PUBLIC LICENSE Version 3](LICENSE) - see the LICENSE file for details.
