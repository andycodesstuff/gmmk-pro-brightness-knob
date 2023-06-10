mod keyboard_knob;

use self::keyboard_knob::register_knob_adjustment_handler;

use std::{thread, time::Duration};

fn main() {
  let mut threads = Vec::new();
  threads.push(thread::spawn(move || { register_knob_adjustment_handler(Some(true)); }));
  threads.push(thread::spawn(move || {
    // Test thread
    loop {
      println!("hello, world!");
      thread::sleep(Duration::from_millis(250));
    }
  }));

  for t in threads { t.join().unwrap(); }
}
