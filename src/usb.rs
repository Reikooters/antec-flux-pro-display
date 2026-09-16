//! The display side: a USB HID device (vendor 0x2022, product 0x0522) that takes a
//! 12-byte frame on its interrupt OUT endpoint. It has no IN endpoint, so the kernel's
//! HID driver does not bind to it and it is driven through libusb.
//!
//! Frame layout, as sent by Antec's own software:
//!
//! ```text
//! 55 AA 01 01 06  c c c  g g g  sum
//! ```
//!
//! `c` and `g` are the tens, ones and tenths digits of the CPU and GPU temperature;
//! `EE EE EE` shows dashes. `sum` is the 8-bit wrapping sum of the preceding bytes.
//!
//! Adapted from nishtahir/antec-flux-pro-display:
//! <https://github.com/nishtahir/antec-flux-pro-display/blob/main/src/usb.rs>

use std::time::Duration;

use anyhow::{Context, Result, bail};
use rusb::{DeviceHandle, Direction, GlobalContext, TransferType};

const VENDOR_ID: u16 = 0x2022;
const PRODUCT_ID: u16 = 0x0522;

const INTERFACE: u8 = 0;
const FALLBACK_ENDPOINT: u8 = 0x03;
const WRITE_TIMEOUT: Duration = Duration::from_millis(1000);

/// Digit code the panel renders as a dash.
const NO_READING: u8 = 0xEE;

pub struct UsbDevice {
    handle: DeviceHandle<GlobalContext>,
    endpoint: u8,
}

impl UsbDevice {
    /// Open and claim the display. The error says whether it is absent or why it
    /// could not be opened (rusb reports permission and busy errors by name).
    pub fn open() -> Result<Self> {
        let device = rusb::devices()
            .context("listing USB devices")?
            .iter()
            .find(|device| {
                device
                    .device_descriptor()
                    .map(|d| d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID)
                    .unwrap_or(false)
            })
            .with_context(|| {
                format!("display {VENDOR_ID:04x}:{PRODUCT_ID:04x} is not connected")
            })?;
        let handle = device
            .open()
            .with_context(|| format!("opening display {VENDOR_ID:04x}:{PRODUCT_ID:04x}"))?;

        let endpoint = device
            .config_descriptor(0)
            .context("reading the display's configuration descriptor")?
            .interfaces()
            .flat_map(|interface| interface.descriptors())
            .flat_map(|descriptor| descriptor.endpoint_descriptors())
            .find(|endpoint| {
                endpoint.transfer_type() == TransferType::Interrupt
                    && endpoint.direction() == Direction::Out
            })
            .map(|endpoint| endpoint.address())
            .unwrap_or(FALLBACK_ENDPOINT);

        let display = Self { handle, endpoint };
        display.claim()?;
        Ok(display)
    }

    /// Claim the interface, defensively detaching a kernel driver should one ever
    /// have bound to it.
    fn claim(&self) -> Result<()> {
        if self.handle.kernel_driver_active(INTERFACE).unwrap_or(false) {
            self.handle
                .detach_kernel_driver(INTERFACE)
                .context("detaching the kernel driver from the display")?;
        }
        self.handle
            .claim_interface(INTERFACE)
            .context("claiming the display's interface")
    }

    /// Send one frame. `None` shows dashes for that temperature.
    pub fn send(&self, cpu: Option<f64>, gpu: Option<f64>) -> Result<()> {
        let frame = frame(cpu, gpu);
        let written = self
            .handle
            .write_interrupt(self.endpoint, &frame, WRITE_TIMEOUT)
            .with_context(|| format!("writing to endpoint {:#04x}", self.endpoint))?;
        if written != frame.len() {
            bail!(
                "short write to endpoint {:#04x}: {written} of {} bytes",
                self.endpoint,
                frame.len()
            );
        }
        Ok(())
    }
}

fn frame(cpu: Option<f64>, gpu: Option<f64>) -> [u8; 12] {
    let mut frame = [0x55, 0xAA, 0x01, 0x01, 0x06, 0, 0, 0, 0, 0, 0, 0];
    frame[5..8].copy_from_slice(&digits(cpu));
    frame[8..11].copy_from_slice(&digits(gpu));
    frame[11] = frame[..11].iter().fold(0u8, |sum, b| sum.wrapping_add(*b));
    frame
}

/// Tens, ones and tenths of a temperature rounded to 0.1 °C and clamped to the
/// panel's range of 00.0 to 99.9.
fn digits(temperature: Option<f64>) -> [u8; 3] {
    match temperature {
        Some(t) if t.is_finite() => {
            let tenths = (t.clamp(0.0, 99.9) * 10.0).round() as u16;
            [
                (tenths / 100) as u8,
                (tenths / 10 % 10) as u8,
                (tenths % 10) as u8,
            ]
        }
        _ => [NO_READING; 3],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_with_both_temperatures() {
        let expected = [85, 170, 1, 1, 6, 2, 4, 0, 1, 6, 0, 20];
        assert_eq!(frame(Some(24.0), Some(16.0)), expected);
    }

    #[test]
    fn frame_without_gpu_shows_dashes() {
        let expected = [85, 170, 1, 1, 6, 2, 4, 0, 238, 238, 238, 215];
        assert_eq!(frame(Some(24.0), None), expected);
    }

    #[test]
    fn digits_round_to_tenths() {
        assert_eq!(digits(Some(45.96)), [4, 6, 0]);
        assert_eq!(digits(Some(59.94)), [5, 9, 9]);
        assert_eq!(digits(Some(0.05)), [0, 0, 1]);
    }

    #[test]
    fn digits_clamp_to_panel_range() {
        assert_eq!(digits(Some(99.95)), [9, 9, 9]);
        assert_eq!(digits(Some(105.3)), [9, 9, 9]);
        assert_eq!(digits(Some(-3.0)), [0, 0, 0]);
        assert_eq!(digits(Some(f64::NAN)), [NO_READING; 3]);
        assert_eq!(digits(Some(f64::INFINITY)), [NO_READING; 3]);
    }
}
