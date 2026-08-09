use std::time::{Duration, Instant};

pub const EXIT_HOLD_DURATION: Duration = Duration::from_millis(2_000);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControllerSample {
    pub connected: bool,
    pub start_pressed: bool,
    pub select_pressed: bool,
}

impl ControllerSample {
    pub const DISCONNECTED: Self = Self {
        connected: false,
        start_pressed: false,
        select_pressed: false,
    };

    fn both_pressed(self) -> bool {
        self.connected && self.start_pressed && self.select_pressed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutEvent {
    ControllerConnected {
        controller_index: usize,
    },
    ControllerDisconnected {
        controller_index: usize,
    },
    HoldStarted {
        controller_index: usize,
    },
    HoldCancelled {
        controller_index: usize,
    },
    ThresholdReached {
        controller_index: usize,
        hold_ms: u64,
    },
    Triggered {
        controller_index: usize,
        hold_ms: u64,
    },
    DuplicateTriggerIgnored {
        controller_index: usize,
    },
    MonitorError {
        code: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShortcutPhase {
    Idle,
    Holding,
    WaitRelease,
}

#[derive(Debug)]
pub struct ShortcutStateMachine {
    phase: ShortcutPhase,
    holding_controller: Option<usize>,
    hold_started_at: Option<Instant>,
    duplicate_logged: bool,
    blocked_controller: Option<usize>,
}

impl Default for ShortcutStateMachine {
    fn default() -> Self {
        Self {
            phase: ShortcutPhase::Idle,
            holding_controller: None,
            hold_started_at: None,
            duplicate_logged: false,
            blocked_controller: None,
        }
    }
}

impl ShortcutStateMachine {
    pub fn update(&mut self, now: Instant, samples: &[ControllerSample; 4]) -> Vec<ShortcutEvent> {
        let mut events = Vec::new();
        match self.phase {
            ShortcutPhase::Idle => {
                if let Some(controller_index) = self.blocked_controller {
                    if !samples[controller_index].both_pressed() {
                        self.blocked_controller = None;
                    } else {
                        return events;
                    }
                }
                if let Some((controller_index, _)) = samples
                    .iter()
                    .enumerate()
                    .find(|(_, sample)| sample.both_pressed())
                {
                    self.phase = ShortcutPhase::Holding;
                    self.holding_controller = Some(controller_index);
                    self.hold_started_at = Some(now);
                    self.duplicate_logged = false;
                    events.push(ShortcutEvent::HoldStarted { controller_index });
                }
            }
            ShortcutPhase::Holding => {
                let Some(controller_index) = self.holding_controller else {
                    self.reset();
                    return events;
                };
                let sample = samples[controller_index];
                if !sample.connected {
                    events.push(ShortcutEvent::HoldCancelled { controller_index });
                    self.reset();
                } else if !sample.both_pressed() {
                    events.push(ShortcutEvent::HoldCancelled { controller_index });
                    self.reset();
                } else if let Some(started_at) = self.hold_started_at {
                    let hold_ms = now.duration_since(started_at).as_millis() as u64;
                    if hold_ms >= EXIT_HOLD_DURATION.as_millis() as u64 {
                        events.push(ShortcutEvent::ThresholdReached {
                            controller_index,
                            hold_ms,
                        });
                        events.push(ShortcutEvent::Triggered {
                            controller_index,
                            hold_ms,
                        });
                        self.phase = ShortcutPhase::WaitRelease;
                    }
                }
            }
            ShortcutPhase::WaitRelease => {
                let Some(controller_index) = self.holding_controller else {
                    self.reset();
                    return events;
                };
                let sample = samples[controller_index];
                if !sample.connected {
                    self.reset();
                } else if !sample.both_pressed() {
                    self.reset();
                } else if !self.duplicate_logged
                    && samples
                        .iter()
                        .enumerate()
                        .any(|(index, other)| index != controller_index && other.both_pressed())
                {
                    self.duplicate_logged = true;
                    events.push(ShortcutEvent::DuplicateTriggerIgnored { controller_index });
                }
            }
        }
        events
    }

    fn reset(&mut self) {
        self.phase = ShortcutPhase::Idle;
        self.holding_controller = None;
        self.hold_started_at = None;
        self.duplicate_logged = false;
    }

    fn block_controller_until_release(&mut self, controller_index: usize) {
        self.reset();
        self.blocked_controller = Some(controller_index);
    }
}

#[derive(Debug)]
pub struct GamepadShortcutMonitor {
    machine: ShortcutStateMachine,
    previous_connected: [bool; 4],
    last_error: Option<u32>,
}

impl Default for GamepadShortcutMonitor {
    fn default() -> Self {
        Self {
            machine: ShortcutStateMachine::default(),
            previous_connected: [false; 4],
            last_error: None,
        }
    }
}

impl GamepadShortcutMonitor {
    pub fn poll(&mut self, now: Instant) -> Vec<ShortcutEvent> {
        let mut events = Vec::new();
        let samples = match read_xinput_samples() {
            Ok(samples) => {
                if self.last_error.take().is_some() {
                    // A successful read is enough to establish reconnection;
                    // the controller transition is emitted below.
                }
                samples
            }
            Err(code) => {
                if self.last_error != Some(code) {
                    self.last_error = Some(code);
                    events.push(ShortcutEvent::MonitorError { code });
                }
                [ControllerSample::DISCONNECTED; 4]
            }
        };
        for (controller_index, sample) in samples.iter().enumerate() {
            if sample.connected && !self.previous_connected[controller_index] {
                events.push(ShortcutEvent::ControllerConnected { controller_index });
                self.machine
                    .block_controller_until_release(controller_index);
            } else if !sample.connected && self.previous_connected[controller_index] {
                events.push(ShortcutEvent::ControllerDisconnected { controller_index });
            }
        }
        self.previous_connected = samples.map(|sample| sample.connected);
        events.extend(self.machine.update(now, &samples));
        events
    }
}

#[cfg(windows)]
fn read_xinput_samples() -> Result<[ControllerSample; 4], u32> {
    use windows_sys::Win32::Foundation::ERROR_DEVICE_NOT_CONNECTED;
    use windows_sys::Win32::UI::Input::XboxController::{
        XInputGetState, XINPUT_GAMEPAD_BACK, XINPUT_GAMEPAD_START, XINPUT_STATE,
    };
    let mut samples = [ControllerSample::DISCONNECTED; 4];
    for (controller_index, sample) in samples.iter_mut().enumerate() {
        let mut state: XINPUT_STATE = unsafe { std::mem::zeroed() };
        let result = unsafe { XInputGetState(controller_index as u32, &mut state) };
        if result == 0 {
            sample.connected = true;
            sample.start_pressed = state.Gamepad.wButtons & XINPUT_GAMEPAD_START != 0;
            sample.select_pressed = state.Gamepad.wButtons & XINPUT_GAMEPAD_BACK != 0;
        } else if result != ERROR_DEVICE_NOT_CONNECTED {
            return Err(result);
        }
    }
    Ok(samples)
}

#[cfg(not(windows))]
fn read_xinput_samples() -> Result<[ControllerSample; 4], u32> {
    Ok([ControllerSample::DISCONNECTED; 4])
}

#[cfg(test)]
mod tests {
    use super::{ControllerSample, ShortcutEvent, ShortcutStateMachine, EXIT_HOLD_DURATION};
    use std::time::{Duration, Instant};

    fn pressed() -> ControllerSample {
        ControllerSample {
            connected: true,
            start_pressed: true,
            select_pressed: true,
        }
    }

    #[test]
    fn start_only_does_not_trigger() {
        let mut machine = ShortcutStateMachine::default();
        let now = Instant::now();
        let sample = ControllerSample {
            connected: true,
            start_pressed: true,
            select_pressed: false,
        };
        let samples = [
            sample,
            ControllerSample::DISCONNECTED,
            ControllerSample::DISCONNECTED,
            ControllerSample::DISCONNECTED,
        ];
        assert!(machine.update(now, &samples).is_empty());
    }

    #[test]
    fn hold_under_threshold_cancels() {
        let mut machine = ShortcutStateMachine::default();
        let now = Instant::now();
        let samples = [
            pressed(),
            ControllerSample::DISCONNECTED,
            ControllerSample::DISCONNECTED,
            ControllerSample::DISCONNECTED,
        ];
        assert!(matches!(
            machine.update(now, &samples).as_slice(),
            [ShortcutEvent::HoldStarted { .. }]
        ));
        assert!(machine
            .update(
                now + EXIT_HOLD_DURATION - Duration::from_millis(1),
                &samples
            )
            .is_empty());
        assert!(matches!(
            machine
                .update(
                    now + EXIT_HOLD_DURATION,
                    &[
                        ControllerSample {
                            select_pressed: false,
                            ..pressed()
                        },
                        ControllerSample::DISCONNECTED,
                        ControllerSample::DISCONNECTED,
                        ControllerSample::DISCONNECTED
                    ]
                )
                .as_slice(),
            [ShortcutEvent::HoldCancelled { .. }]
        ));
    }

    #[test]
    fn long_hold_triggers_once_until_release() {
        let mut machine = ShortcutStateMachine::default();
        let now = Instant::now();
        let samples = [
            pressed(),
            ControllerSample::DISCONNECTED,
            ControllerSample::DISCONNECTED,
            ControllerSample::DISCONNECTED,
        ];
        machine.update(now, &samples);
        let events = machine.update(now + EXIT_HOLD_DURATION, &samples);
        assert!(events
            .iter()
            .any(|event| matches!(event, ShortcutEvent::Triggered { .. })));
        assert!(machine
            .update(now + EXIT_HOLD_DURATION + Duration::from_secs(5), &samples)
            .is_empty());
        machine.update(
            now + EXIT_HOLD_DURATION + Duration::from_secs(5),
            &[ControllerSample::DISCONNECTED; 4],
        );
        assert!(machine
            .update(now + EXIT_HOLD_DURATION + Duration::from_secs(6), &samples)
            .iter()
            .any(|event| matches!(event, ShortcutEvent::HoldStarted { .. })));
    }

    #[test]
    fn second_controller_cannot_complete_first_controller_hold() {
        let mut machine = ShortcutStateMachine::default();
        let now = Instant::now();
        let first = [
            pressed(),
            ControllerSample::DISCONNECTED,
            ControllerSample::DISCONNECTED,
            ControllerSample::DISCONNECTED,
        ];
        let second = [
            ControllerSample {
                connected: true,
                start_pressed: true,
                select_pressed: false,
            },
            pressed(),
            ControllerSample::DISCONNECTED,
            ControllerSample::DISCONNECTED,
        ];
        machine.update(now, &first);
        let events = machine.update(now + EXIT_HOLD_DURATION, &second);
        assert!(events.iter().any(|event| matches!(
            event,
            ShortcutEvent::HoldCancelled {
                controller_index: 0
            }
        )));
        assert!(!events
            .iter()
            .any(|event| matches!(event, ShortcutEvent::Triggered { .. })));
    }

    #[test]
    fn reconnect_requires_release_before_a_new_hold() {
        let mut machine = ShortcutStateMachine::default();
        let now = Instant::now();
        let pressed_samples = [
            pressed(),
            ControllerSample::DISCONNECTED,
            ControllerSample::DISCONNECTED,
            ControllerSample::DISCONNECTED,
        ];
        machine.block_controller_until_release(0);
        assert!(machine.update(now, &pressed_samples).is_empty());
        let released_samples = [
            ControllerSample {
                connected: true,
                start_pressed: false,
                select_pressed: false,
            },
            ControllerSample::DISCONNECTED,
            ControllerSample::DISCONNECTED,
            ControllerSample::DISCONNECTED,
        ];
        machine.update(now + Duration::from_millis(10), &released_samples);
        assert!(machine
            .update(now + Duration::from_millis(20), &pressed_samples)
            .iter()
            .any(|event| matches!(event, ShortcutEvent::HoldStarted { .. })));
    }
}
