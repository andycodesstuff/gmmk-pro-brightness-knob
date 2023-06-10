use ctrlc;
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
  CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
  HHOOK, MSG, MSLLHOOKSTRUCT, WINDOWS_HOOK_ID, WM_QUIT
};

const HC_ACTION: i32 = 0;
const WH_MOUSE_LL: WINDOWS_HOOK_ID = WINDOWS_HOOK_ID(14);
const WM_MOUSEWHEEL: WPARAM = WPARAM(522usize);

/// Register the event handler for adjustments to the knob. These adjustments can come either from the physical keyboard
/// device, or emulated using the vertical mouse scroll wheel
pub fn register_knob_adjustment_handler(emulate_knob: Option<bool>) {
  if emulate_knob.is_some() {
    unsafe { emulate_knob_with_mousewheel(); }
  } else {
    todo!("detect keyboard knob adjustments")
  }
}

/// Emulate the keyboard knob using the vertical mouse scroll wheel
unsafe fn emulate_knob_with_mousewheel() {
  let thread_id = GetCurrentThreadId();

  // Register a hook for capturing low-level mouse input events
  let hook_res = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), HMODULE(0), 0);
  if let Err(hook_err) = hook_res {
    println!("ERROR: failed to register a hook for low-level mouse input events - code: {}", hook_err);
    return;
  }

  // Unwrap the actual hook ID once we handled the error case
  let hook_id = hook_res.unwrap();

  // Register a Ctrl-C handler to signal when to stop listening for mouse input events
  let handler_res = ctrlc::set_handler(move || {
    println!("INFO: received Ctrl-C, stopping the hook...");

    // Send the WM_QUIT message to the main thread so that GetMessageW can return and exit the program gracefully
    // Note: PostQuitMessage won't work here because we are on a different thread!
    PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
  });
  if let Err(handler_err) = handler_res {
    println!("ERROR: failed to register Ctrl-C handler - code: {}", handler_err);

    UnhookWindowsHookEx(hook_id);
    return;
  }

  // Listen to mouse input events
  let mut msg: MSG = Default::default();
  while GetMessageW(&mut msg, HWND(0), 0, 0).as_bool() {
    TranslateMessage(&msg);
    DispatchMessageW(&msg);
  }

  UnhookWindowsHookEx(hook_id);
}

/// Handle low-level mouse input events
/// 
/// Note: When a WH_MOUSE_LL hook is registered, all the data is stored in a MSLLHOOKSTRUCT struct pointed by the LPARAM
///       argument
/// Reference: https://stackoverflow.com/a/68827449
unsafe extern "system" fn mouse_hook(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
  if code == HC_ACTION && w_param == WM_MOUSEWHEEL {
    // Dereference the pointer to get the mouse input event data, then extract the high-order word (the first 2 bytes)
    // of the mouseData member to get the mouse delta. After casting it to a short int, a positive value indicates that
    // the wheel was rotated forward, away from the user; a negative value indicates that the wheel was rotated
    // backward, towards the user
    // 
    // Reference: https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-msllhookstruct#members
    let mouse_event = *(l_param.0 as *const MSLLHOOKSTRUCT);
    let mouse_delta = ((mouse_event.mouseData >> 16) & 0xffff) as u16 as i16;

    println!("mouse delta: {:?} {}", mouse_event, mouse_delta);
  }

  CallNextHookEx(HHOOK(0), code, w_param, l_param)
}
