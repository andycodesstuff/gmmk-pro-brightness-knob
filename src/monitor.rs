use ddc::{Ddc, FeatureCode};
use ddc_winapi::get_physical_monitors_from_hmonitor;
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTOPRIMARY};

const BRIGHTNESS_VCP_CODE: FeatureCode = 0x10;
const POINT_ZERO: POINT = POINT { x: 0, y: 0 };

/// Represent a monitor connected to the PC
pub struct Monitor {
  ddc_handle: ddc_winapi::Monitor,
  pub refresh_rate_hz: u16
}

impl Monitor {
  /// Create a new struct using the primary monitor info
  pub fn new_primary() -> Self {
    // Get the handle to the primary monitor. By definition, the primary monitor has its upper-left corner at (0, 0)
    let hmonitor_handle = unsafe { MonitorFromPoint(POINT_ZERO, MONITOR_DEFAULTTOPRIMARY) };
    let physical_handle = get_physical_monitors_from_hmonitor(hmonitor_handle.0 as *mut _).unwrap()[0];
  
    let mut ddc_handle = unsafe { ddc_winapi::Monitor::new(physical_handle) };
    let refresh_rate_hz = match ddc_handle.get_timing_report() {
      Ok(report) => report.vertical_frequency / 100,
      _ => 60u16
    };
  
    Self {
      ddc_handle,
      refresh_rate_hz
    }
  }

  /// Get the brightness of the current monitor, or fetches the primary monitor first to get the most up-to-date one
  pub fn get_brightness(&mut self) -> u16 {
    // The current monitor brightness is held in the low byte of the VCP value
    let value = self.ddc_handle.get_vcp_feature(BRIGHTNESS_VCP_CODE).unwrap();
    value.sl as u16
  }

  pub fn set_brightness(&mut self, value: u16) {
    self.ddc_handle.set_vcp_feature(BRIGHTNESS_VCP_CODE, value).unwrap();
  }
}
