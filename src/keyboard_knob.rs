use crossbeam_channel::Sender;
use ctrlc;
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{VIRTUAL_KEY, VK_F19, VK_F20};
use windows::Win32::UI::WindowsAndMessaging::{
  CallNextHookEx, DispatchMessageW, GetMessageW, PostMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
  HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WINDOWS_HOOK_ID, WM_KEYUP, WM_QUIT, WM_SYSKEYUP
};

const HC_ACTION: i32 = 0;
const WH_KEYBOARD_LL: WINDOWS_HOOK_ID = WINDOWS_HOOK_ID(13);
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
pub fn register_knob_adjustment_handler(channel_tx: Sender<KnobAdjustmentEvent>, emulate_knob: Option<bool>) -> Result<(), HandlerError> {
  unsafe {
    let thread_id = GetCurrentThreadId();

    // Register a hook for capturing low-level input events
    let hook_id = match emulate_knob {
      Some(_) => SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), HMODULE(0), 0)?,
      None => SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), HMODULE(0), 0)?
    };

    // Register a Ctrl-C handler to signal when to stop listening for input events
    let handler_res = ctrlc::set_handler(move || {
      println!("INFO: received Ctrl-C, stopping the input event listener...");

      // Send the WM_QUIT message to the main thread so that GetMessageW can return and exit the program gracefully
      // Note: PostQuitMessage won't work here because we are on a different thread!
      PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
    });
    if let Err(handler_err) = handler_res {
      UnhookWindowsHookEx(hook_id);
      return Err(HandlerError::StopHandlerError(handler_err));
    }

    // Message loop
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
}

/// Handle low-level keyboard input events
/// 
/// Note: A WH_KEYBOARD_LL hook stores the input event data in a KBDLLHOOKSTRUCT struct pointed by the LPARAM argument
/// Reference: https://learn.microsoft.com/en-us/previous-versions/windows/desktop/legacy/ms644985(v=vs.85)#parameters
unsafe extern "system" fn keyboard_hook(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
  if code != HC_ACTION {
    return CallNextHookEx(HHOOK(0), code, w_param, l_param);
  }

  // Dereference the pointer to get the keyboard input event data
  let keyboard_event = *(l_param.0 as *const KBDLLHOOKSTRUCT);
  let key_code = VIRTUAL_KEY(keyboard_event.vkCode as u16);

  // The identifier of the keyboard message is simply stored in the WPARAM argument
  let key_state = w_param.0 as u32;
  let is_key_up = key_state == WM_KEYUP || key_state == WM_SYSKEYUP;
  if !is_key_up {
    return CallNextHookEx(HHOOK(0), code, w_param, l_param);
  }

  // Send the parsed keyboard event back to the message loop
  if let Some(msg) = match key_code {
    VK_F19 => Some(KnobAdjustmentEvent::Decrement),
    VK_F20 => Some(KnobAdjustmentEvent::Increment),
    _ => None
  } {
    PostMessageW(HWND(0), msg as u32, WPARAM(0), LPARAM(0));
  }
  CallNextHookEx(HHOOK(0), code, w_param, l_param)
}

/// Handle low-level mouse input events
/// 
/// Note: A WH_MOUSE_LL hook stores the input event data in a MSLLHOOKSTRUCT struct pointed by the LPARAM argument
/// Reference: https://stackoverflow.com/a/68827449
unsafe extern "system" fn mouse_hook(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
  if code != HC_ACTION || w_param != WM_MOUSEWHEEL {
    return CallNextHookEx(HHOOK(0), code, w_param, l_param);
  }

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
  CallNextHookEx(HHOOK(0), code, w_param, l_param)
}

#[derive(Debug)]
pub enum HandlerError {
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
