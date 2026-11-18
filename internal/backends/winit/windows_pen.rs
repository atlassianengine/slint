// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Windows Ink pressure fallback for already-delivered Winit touch events.
//!
//! Some drivers report a pen contact but do not include a Force field in the
//! Winit Touch event. Query the current Windows pointer's PEN_MASK_PRESSURE,
//! not button state/velocity or old buffered samples. This does NOT install a
//! WndProc hook or claim to support mouse-only compatibility pen devices.
//! A Winit touch id is not guaranteed to equal the Win32 pointer id on every
//! backend; GetPointerPenInfo failure must cleanly mean "no measurement".

#![cfg(target_os = "windows")]

use std::ffi::c_void;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::Pointer::{
    GetPointerPenInfo, POINTER_PEN_INFO,
};

/// Maximum physical-pixel disagreement permitted between the Winit client
/// touch and Win32's live pointer position. A mismatched pointer ID can
/// otherwise accidentally return another active tablet's pressure.
const MAX_POINTER_POSITION_ERROR_PX: f64 = 4.0;

/// Win32 defines the maximum pen contact pressure as 1024. Reject malformed
/// samples instead of interpreting arbitrary raw integers as normalized force.
#[inline]
fn normalized_windows_pressure(raw: u32) -> Option<f32> {
    (raw <= 1024).then_some(raw as f32 / 1024.0)
}

pub(crate) fn normalized_pressure_for_touch_id(
    touch_id: u64,
    window: &winit::window::Window,
    client_position_px: winit::dpi::PhysicalPosition<f64>,
) -> Option<f32> {
    let pointer_id = u32::try_from(touch_id).ok()?;
    let RawWindowHandle::Win32(handle) = window.window_handle().ok()?.as_raw()
        else { return None };
    let hwnd = HWND(handle.hwnd.get() as *mut c_void);
    let client_origin = window.inner_position().ok()?;
    let mut pen = POINTER_PEN_INFO::default();
    // Safety: GetPointerPenInfo writes into this initialized C-ABI output
    // structure and does not retain the pointer. It is queried only during
    // the Winit touch event, while the OS pointer ID may still be live.
    // A failed query or non-pen touch cannot authorize fabricated pressure.
    if unsafe { GetPointerPenInfo(pointer_id, &mut pen) }.is_err() {
        return None;
    }
    // A Winit touch id is not necessarily a native Win32 pointer ID.
    // Prove the source window AND client contact location before importing
    // force to another input sequence. Failure is an honest no-measurement.
    if pen.pointerInfo.hwndTarget != hwnd {
        return None;
    }
    let reported = pen.pointerInfo.ptPixelLocation;
    let reported_x = f64::from(reported.x) - f64::from(client_origin.x);
    let reported_y = f64::from(reported.y) - f64::from(client_origin.y);
    if !client_position_px.x.is_finite() || !client_position_px.y.is_finite()
        || (reported_x - client_position_px.x).abs() > MAX_POINTER_POSITION_ERROR_PX
        || (reported_y - client_position_px.y).abs() > MAX_POINTER_POSITION_ERROR_PX
    {
        return None;
    }
    // windows-rs 0.62 models penMask as raw u32. Win32 PEN_MASK_PRESSURE = 0x0001.
    const PEN_PRESSURE_PRESENT: u32 = 0x0001;
    if (pen.penMask & PEN_PRESSURE_PRESENT) == 0 {
        return None;
    }
    normalized_windows_pressure(pen.pressure)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressure_normalization_is_exactly_bounded() {
        assert_eq!(normalized_windows_pressure(0), Some(0.0));
        assert_eq!(normalized_windows_pressure(256), Some(0.25));
        assert_eq!(normalized_windows_pressure(512), Some(0.5));
        assert_eq!(normalized_windows_pressure(1024), Some(1.0));
        assert_eq!(normalized_windows_pressure(1025), None);
        assert_eq!(normalized_windows_pressure(u32::MAX), None);
    }
}
