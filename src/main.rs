mod keyboard_knob;

use self::keyboard_knob::register_knob_adjustment_handler;

fn main() {
  register_knob_adjustment_handler(Some(true));
}
