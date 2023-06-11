use crossbeam_channel::Sender;
use ctrlc;
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
  CallNextHookEx, DispatchMessageW, GetMessageW, PostMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
  HHOOK, MSG, MSLLHOOKSTRUCT, WINDOWS_HOOK_ID, WM_QUIT
};

const HC_ACTION: i32 = 0;
const WH_MOUSE_LL: WINDOWS_HOOK_ID = WINDOWS_HOOK_ID(14);
const WM_MOUSEWHEEL: WPARAM = WPARAM(522usize);

/// Represent a knob adjustment event. The values chosen for the enum items are not random, and were chosen according
/// to Microsoft's documentation on application-defined messages
/// 
/// Reference: https://learn.microsoft.com/en-us/windows/win32/winmsg/about-messages-and-message-queues#application-defined-messages
#[repr(u32)]
pub enum KnobAdjustmentEvent {
  Increment = 0x0500,
  Decrement = 0x0502
}

/// Register the event handler for adjustments to the knob. These adjustments can come either from the physical keyboard
/// device, or emulated using the vertical mouse scroll wheel
pub fn register_knob_adjustment_handler(channel_tx: Sender<KnobAdjustmentEvent>, emulate_knob: Option<bool>) {
  if emulate_knob.is_some() {
    unsafe {
      emulate_knob_with_mousewheel(channel_tx).unwrap_or_else(|err| {
        match err {
          HandlerError::HookError(e) => eprintln!("ERROR: failed to register a hook for low-level mouse input events - code: {}", e),
          HandlerError::StopHandlerError(e) => eprintln!("ERROR: failed to register Ctrl-C handler - code: {}", e),
          HandlerError::TXError(_) => eprintln!("ERROR: unable to forward knob adjustment events to the other threads")
        };
      });
    }
  } else {
    todo!("detect keyboard knob adjustments")
  }
}

/// Emulate the keyboard knob using the vertical mouse scroll wheel
unsafe fn emulate_knob_with_mousewheel(channel_tx: Sender<KnobAdjustmentEvent>) -> Result<(), HandlerError> {
  let thread_id = GetCurrentThreadId();

  // Register a hook for capturing low-level mouse input events
  let hook_id = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), HMODULE(0), 0)?;

  // Register a Ctrl-C handler to signal when to stop listening for mouse input events
  let handler_res = ctrlc::set_handler(move || {
    println!("INFO: received Ctrl-C, stopping the hook...");

    // Send the WM_QUIT message to the main thread so that GetMessageW can return and exit the program gracefully
    // Note: PostQuitMessage won't work here because we are on a different thread!
    PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
  });
  if let Err(handler_err) = handler_res {
    UnhookWindowsHookEx(hook_id);
    return Err(HandlerError::StopHandlerError(handler_err));
  }

  // Listen to mouse input events
  let mut msg: MSG = Default::default();
  while GetMessageW(&mut msg, HWND(0), 0, 0).as_bool() {
    TranslateMessage(&msg);

    // Forward the knob adjustment events to the other thread(s)
    let evt = msg.message;
    match evt {
      evt if evt == KnobAdjustmentEvent::Increment as u32 => channel_tx.send(KnobAdjustmentEvent::Increment)?,
      evt if evt == KnobAdjustmentEvent::Decrement as u32 => channel_tx.send(KnobAdjustmentEvent::Decrement)?,
      _ => {}
    };

    DispatchMessageW(&msg);
  }

  UnhookWindowsHookEx(hook_id);
  Ok(())
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

    // Send the parsed mouse event back to the message loop
    let msg = (if mouse_delta > 0 { KnobAdjustmentEvent::Increment } else { KnobAdjustmentEvent::Decrement }) as u32;
    PostMessageW(HWND(0), msg, WPARAM(0), LPARAM(0));
  }

  CallNextHookEx(HHOOK(0), code, w_param, l_param)
}

#[derive(Debug)]
enum HandlerError {
  HookError(windows::core::Error),
  StopHandlerError(ctrlc::Error),
  TXError(crossbeam_channel::SendError<KnobAdjustmentEvent>)
}

impl From<windows::core::Error> for HandlerError {
  fn from(value: windows::core::Error) -> Self {
    HandlerError::HookError(value)
  }
}

impl From<crossbeam_channel::SendError<KnobAdjustmentEvent>> for HandlerError {
  fn from(value: crossbeam_channel::SendError<KnobAdjustmentEvent>) -> Self {
    HandlerError::TXError(value)
  }
}
