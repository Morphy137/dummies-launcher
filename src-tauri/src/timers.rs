//! Local quest clocks, phase snapshots, cancellation and generation-safe completion.

use crate::models::{ExitedPayload, QuestTick};
use crate::processes::stop_running_dummy;
use crate::state::AppState;
use crate::tray::update_tray_menu;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

pub(crate) struct QuestClock {
    started: Instant,
    paused_at: Option<Instant>,
    paused_total: Duration,
}

impl QuestClock {
    pub(crate) fn new() -> Self {
        Self {
            started: Instant::now(),
            paused_at: None,
            paused_total: Duration::ZERO,
        }
    }

    fn elapsed(&self) -> Duration {
        let now = Instant::now();
        let paused_now = self
            .paused_at
            .map(|paused_at| now.saturating_duration_since(paused_at))
            .unwrap_or_default();
        now.saturating_duration_since(self.started)
            .saturating_sub(self.paused_total + paused_now)
    }

    fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }

    pub(crate) fn pause(&mut self) -> bool {
        if self.paused_at.is_some() {
            return false;
        }
        self.paused_at = Some(Instant::now());
        true
    }

    pub(crate) fn resume(&mut self) -> bool {
        let Some(paused_at) = self.paused_at.take() else {
            return false;
        };
        self.paused_total += paused_at.elapsed();
        true
    }
}

pub(crate) struct QuestTimerConfig {
    pub(crate) target_minutes: u32,
    pub(crate) buffer_seconds: u32,
    pub(crate) auto_stop: bool,
    pub(crate) notify: bool,
    pub(crate) spanish: bool,
}

pub(crate) struct QuestTimerHandles {
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) clock: Arc<Mutex<QuestClock>>,
    pub(crate) last_tick: Arc<Mutex<Option<QuestTick>>>,
}

pub(crate) fn start_quest_timer(
    app: AppHandle,
    key: String,
    name: String,
    timer_id: u64,
    config: QuestTimerConfig,
) -> QuestTimerHandles {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_signal = stop.clone();
    let clock = Arc::new(Mutex::new(QuestClock::new()));
    let clock_signal = clock.clone();
    let last_tick = Arc::new(Mutex::new(None));
    let tick_signal = last_tick.clone();

    thread::spawn(move || {
        let target = u64::from(config.target_minutes) * 60;
        let total = target + u64::from(config.buffer_seconds);

        loop {
            if stop_signal.load(Ordering::SeqCst) {
                break;
            }

            let (elapsed, paused) = {
                let clock = clock_signal
                    .lock()
                    .expect("quest clock should not be poisoned");
                (clock.elapsed(), clock.is_paused())
            };
            let elapsed = elapsed.as_secs();
            let remaining = total.saturating_sub(elapsed);
            let phase = if elapsed >= total {
                "complete"
            } else if elapsed >= target {
                "syncing"
            } else {
                "playing"
            };
            let percent = (elapsed.min(total) * 100).checked_div(total).unwrap_or(100) as u8;

            let state = app.state::<AppState>();
            let active = state
                .running
                .lock()
                .map(|running| {
                    running
                        .get(&key)
                        .and_then(|dummy| dummy.quest_clock.as_ref())
                        .is_some_and(|current| Arc::ptr_eq(current, &clock_signal))
                })
                .unwrap_or(false);
            if stop_signal.load(Ordering::SeqCst) {
                break;
            }
            if !active {
                thread::sleep(Duration::from_millis(50));
                continue;
            }

            let tick = QuestTick {
                key: key.clone(),
                name: name.clone(),
                timer_id,
                remaining_seconds: remaining,
                phase: phase.to_string(),
                percent,
                paused,
            };
            if let Ok(mut current) = tick_signal.lock() {
                *current = Some(tick.clone());
            }
            let _ = app.emit("quest://tick", tick);

            if phase == "complete" {
                if config.notify {
                    let _ = app
                        .notification()
                        .builder()
                        .title(if config.spanish {
                            "Temporizador completado"
                        } else {
                            "Timer complete"
                        })
                        .body(if config.spanish {
                            format!("{name} alcanzó el tiempo objetivo local.")
                        } else {
                            format!("{name} reached the local timer goal.")
                        })
                        .show();
                }

                if config.auto_stop {
                    let state = app.state::<AppState>();
                    let dummy = state.running.lock().ok().and_then(|mut running| {
                        if running
                            .get(&key)
                            .and_then(|dummy| dummy.quest_clock.as_ref())
                            .is_some_and(|current| Arc::ptr_eq(current, &clock_signal))
                        {
                            running.remove(&key)
                        } else {
                            None
                        }
                    });
                    if let Some(dummy) = dummy {
                        stop_running_dummy(dummy);
                        let _ = app.emit(
                            "dummy://exited",
                            ExitedPayload {
                                key: key.clone(),
                                name: name.clone(),
                                code: None,
                            },
                        );
                        update_tray_menu(&app, state.inner());
                    }
                }
                break;
            }

            thread::sleep(Duration::from_secs(1));
        }
    });

    QuestTimerHandles {
        stop,
        clock,
        last_tick,
    }
}

fn set_dummy_paused(state: &AppState, game_key: &str, paused: bool) -> Result<bool, String> {
    let running = state.running.lock().map_err(|error| error.to_string())?;
    let dummy = running
        .get(game_key)
        .ok_or_else(|| "The dummy is not running.".to_string())?;
    let Some(clock) = dummy.quest_clock.as_ref() else {
        return Ok(false);
    };
    let mut clock = clock.lock().map_err(|error| error.to_string())?;
    Ok(if paused {
        clock.pause()
    } else {
        clock.resume()
    })
}

pub(crate) fn pause_dummy(state: &AppState, game_key: String) -> Result<bool, String> {
    set_dummy_paused(state, &game_key, true)
}

pub(crate) fn resume_dummy(state: &AppState, game_key: String) -> Result<bool, String> {
    set_dummy_paused(state, &game_key, false)
}
