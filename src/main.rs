mod keyboard_knob;

use self::keyboard_knob::{KnobAdjustmentEvent, register_knob_adjustment_handler};

use std::sync::mpsc;
use std::thread;

fn main() {
  let (tx, rx) = mpsc::channel::<KnobAdjustmentEvent>();

  let mut threads = Vec::new();
  threads.push(thread::spawn(move || { register_knob_adjustment_handler(tx, Some(true)); }));
  threads.push(thread::spawn(move || {
    for received in rx {
      match received {
        KnobAdjustmentEvent::Increment => println!("+++"),
        KnobAdjustmentEvent::Decrement => println!("---")
      };
    }
  }));

  for t in threads { t.join().unwrap(); }
}
