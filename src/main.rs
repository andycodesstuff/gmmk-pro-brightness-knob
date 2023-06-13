mod keyboard_knob;
mod monitor;

use self::keyboard_knob::{KnobAdjustmentEvent, register_knob_adjustment_handler};
use self::monitor::Monitor;

use crossbeam_channel::{Receiver, unbounded};
use keyframe::{ease, functions::EaseInOutCubic};
use std::cmp::{max, min};
use std::hint;
use std::thread;
use std::time::{Duration, Instant};

const ANIM_DURATION: Duration = Duration::from_millis(1000);
const MIN_BRIGHTNESS: i32 = 0;
const MAX_BRIGHTNESS: i32 = 100;

fn main() {
  let (s, r) = unbounded::<KnobAdjustmentEvent>();
  let r_clone = r.clone();

  let mut threads = Vec::new();
  threads.push(thread::spawn(move || { register_knob_adjustment_handler(s, Some(true)); }));
  threads.push(thread::spawn(move || {
    let mut primary_monitor = Monitor::new_primary();
    let mut curr_brightness = primary_monitor.get_brightness() as i32;
    let mut next_brightness = curr_brightness;

    for received in r {
      next_brightness = match received {
        KnobAdjustmentEvent::Increment => min(next_brightness + 1, MAX_BRIGHTNESS),
        KnobAdjustmentEvent::Decrement => max(next_brightness - 1, MIN_BRIGHTNESS) 
      };

      // Avoid unnecessary calls
      if next_brightness != curr_brightness {
        curr_brightness = match adjust_brightness(&mut primary_monitor, &r_clone, curr_brightness, next_brightness, ANIM_DURATION) {
          Err(_) => curr_brightness,
          Ok(value) => value
        };
      }
    }
  }));

  for t in threads { t.join().unwrap(); }
}

/// Adjust the brightness of the monitor by smoothly transitioning from the previous value. If a new knob adjustment
/// event comes through while busy-waiting for the next frame, the transition is interrupted before finishing and the
/// new event takes priority
fn adjust_brightness(monitor: &mut Monitor, r: &Receiver<KnobAdjustmentEvent>, prev_value: i32, target_value: i32, transition_duration: Duration) -> Result<i32, Box<dyn std::error::Error>> {
  let from_brightness = prev_value as f64;
  let to_brightness = target_value as f64;

  // Compute the number of frames required to smoothly transition to the next brightness value in the given duration
  let refresh_rate = monitor.refresh_rate_hz as f32;
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
      monitor.set_brightness(next_brightness as u16);
    }

    // Delays next iteration by a precise time interval
    // Reference: https://stackoverflow.com/a/72837005
    let time = Instant::now();
    while time.elapsed() < frame_time_ms {
      // Interrupt the transition if a new knob adjustment event was registered
      if !r.is_empty() { return Ok(prev_value); }

      hint::spin_loop();
    }

    prev_brightness = next_brightness;
  }

  Ok(target_value)
}
