mod keyboard_knob;

use self::keyboard_knob::{KnobAdjustmentEvent, register_knob_adjustment_handler};

use keyframe::{ease, functions::EaseInOutCubic};
use std::cmp::{max, min};
use std::hint;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const ANIM_DURATION: Duration = Duration::from_millis(1000);
const MIN_BRIGHTNESS: i32 = 0;
const MAX_BRIGHTNESS: i32 = 100;

fn main() {
  let (tx, rx) = mpsc::channel::<KnobAdjustmentEvent>();

  let mut threads = Vec::new();
  threads.push(thread::spawn(move || { register_knob_adjustment_handler(tx, Some(true)); }));
  threads.push(thread::spawn(move || {
    let mut current_monitor_brightness = 0;
    let mut prev_brightness = 0;
    let mut next_brightness = 0;

    for received in rx {
      next_brightness = match received {
        KnobAdjustmentEvent::Increment => min(next_brightness + 1, MAX_BRIGHTNESS),
        KnobAdjustmentEvent::Decrement => max(next_brightness - 1, MIN_BRIGHTNESS) 
      };

      // Avoid unnecessary calls
      if next_brightness != prev_brightness {
        println!("brightness: next {}\tprev {}", next_brightness, prev_brightness);
        current_monitor_brightness = adjust_brightness(current_monitor_brightness, next_brightness, ANIM_DURATION);
      }

      prev_brightness = next_brightness;
    }
  }));

  for t in threads { t.join().unwrap(); }
}

/// Adjust the brightness of the monitor by smoothly transitioning from the previous value
fn adjust_brightness(prev_value: i32, target_value: i32, transition_duration: Duration) -> i32 {
  let from_brightness = prev_value as f64;
  let to_brightness = target_value as f64;

  // Compute the number of frames required to smoothly transition to the next brightness value in the given duration
  let refresh_rate = 165.0;
  let n_frames = max(((transition_duration.as_millis() as f32 * refresh_rate) / 1000.0).ceil() as i32, 1);

  let frame_time_ms = Duration::from_millis(((1.0 / refresh_rate) * 1000.0).floor() as u64);
  let mut prev_brightness = -1;

  for frame in 1..=n_frames {
    // Ease to the target brightness
    let t = frame as f64 / n_frames as f64;
    let next_brightness = ease(EaseInOutCubic, from_brightness, to_brightness, t);
    let next_brightness = (if from_brightness < to_brightness { next_brightness.ceil() } else { next_brightness.floor() }) as i32;

    // Avoid unnecessary updates
    if next_brightness != prev_brightness {
      println!("frame #{}\tvalue {}\tt {}", frame, next_brightness, t);
    }

    // Delays next iteration by a precise time interval
    // Reference: https://stackoverflow.com/a/72837005
    let time = Instant::now();
    while time.elapsed() < frame_time_ms { hint::spin_loop(); }

    prev_brightness = next_brightness;
  }

  target_value
}
