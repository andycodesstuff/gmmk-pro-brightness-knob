use ctrlc;
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
  CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
  HHOOK, MSG, WINDOWS_HOOK_ID, WM_QUIT
};

const WH_MOUSE_LL: WINDOWS_HOOK_ID = WINDOWS_HOOK_ID(14);

unsafe extern "system" fn mouse_hook(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
  println!("mouse event");

  CallNextHookEx(HHOOK(0), code, w_param, l_param)
}

fn main() {  
  unsafe {
    let main_thread_id = GetCurrentThreadId();

    // Register a hook for capturing low-level mouse input events
    let hook_res = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), HMODULE(0), 0);
    if let Err(hook_err) = hook_res {
      println!("ERROR: failed to register a hook for low-level mouse input events - code: {}", hook_err);
      return;
    }

    // Unwrap the actual hook ID once we handled the error case
    let hook_id = hook_res.unwrap();

    // Register a Ctrl-C handler to signal where to stop listening for mouse input events
    let handler_res = ctrlc::set_handler(move || {
      println!("INFO: received Ctrl-C, exiting the program...");

      // Send the WM_QUIT message to the main thread so that GetMessageW can return and exit the program gracefully
      // Note: PostQuitMessage won't work here because we are on a different thread!
      PostThreadMessageW(main_thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
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
}
