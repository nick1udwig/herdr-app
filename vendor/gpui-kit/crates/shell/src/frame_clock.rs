//! HERDR: one clock for everything drawn again on a schedule: spinners,
//! looping animations and the host's own (a terminal's output). Wakes fall on
//! a shared grid of ticks and every wake due at a tick runs in the same task,
//! so they share one window frame instead of each taking its own.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::{App, EntityId, Global};

/// The grid: 30 ticks a second.
pub const TICK: Duration = Duration::from_micros(33_333);

type Wake = Box<dyn FnOnce(&mut App)>;

#[derive(Default)]
struct FrameClock {
    epoch: Option<Instant>,
    /// Per key, the tick it runs at and what it does then.
    due: HashMap<EntityId, (u64, Wake)>,
    /// The tick the clock's timer is set for.
    armed: Option<u64>,
}

impl Global for FrameClock {}

/// Runs `wake` at the next tick that is a multiple of `after` in whole ticks
/// (at least one): every wake of one step (100 ms: every third tick) lands on
/// the same ticks, however far apart they were asked for. A key waits once:
/// asked again before it runs, it keeps the earlier of the two ticks (and the
/// first `wake`, unless the new one is earlier).
pub fn wake(key: EntityId, after: Duration, cx: &mut App, wake: impl FnOnce(&mut App) + 'static) {
    let now = cx.background_executor().now();
    let clock = cx.default_global::<FrameClock>();
    let epoch = *clock.epoch.get_or_insert(now);
    let current = now.saturating_duration_since(epoch).as_nanos() / TICK.as_nanos();
    let ticks = ((after.as_nanos() + TICK.as_nanos() / 2) / TICK.as_nanos()).max(1);
    let tick = ((current / ticks + 1) * ticks) as u64;
    schedule(key, tick, epoch, now, cx, wake);
}

/// Runs `wake` on the first shared tick at or after `deadline`, never before
/// the next change is due. Pending wakes use the same key rules as `wake`.
pub fn wake_at(key: EntityId, deadline: Instant, cx: &mut App, wake: impl FnOnce(&mut App) + 'static) {
    let now = cx.background_executor().now();
    let clock = cx.default_global::<FrameClock>();
    let epoch = *clock.epoch.get_or_insert(now);
    let tick = deadline.saturating_duration_since(epoch).as_nanos().div_ceil(TICK.as_nanos()) as u64;
    let current = now.saturating_duration_since(epoch).as_nanos() / TICK.as_nanos();
    schedule(key, tick.max(current as u64 + 1), epoch, now, cx, wake);
}

fn schedule(key: EntityId, tick: u64, epoch: Instant, now: Instant, cx: &mut App, wake: impl FnOnce(&mut App) + 'static) {
    let clock = cx.global_mut::<FrameClock>();
    match clock.due.get(&key) {
        Some((pending, _)) if *pending <= tick => return,
        _ => {
            clock.due.insert(key, (tick, Box::new(wake)));
        }
    }
    if clock.armed.is_none_or(|armed| armed > tick) {
        arm(tick, epoch, now, cx);
    }
}

fn arm(tick: u64, epoch: Instant, now: Instant, cx: &mut App) {
    cx.global_mut::<FrameClock>().armed = Some(tick);
    let delay = (epoch + TICK * tick as u32).saturating_duration_since(now);
    cx.spawn(async move |cx| {
        cx.background_executor().timer(delay).await;
        cx.update(|cx| fire(tick, cx));
    })
    .detach();
}

fn fire(tick: u64, cx: &mut App) {
    let clock = cx.global_mut::<FrameClock>();
    // A wake asked for an earlier tick moved the timer; that one runs these.
    if clock.armed != Some(tick) {
        return;
    }
    clock.armed = None;
    let due: Vec<EntityId> = clock.due.iter().filter(|(_, (at, _))| *at <= tick).map(|(key, _)| *key).collect();
    let wakes: Vec<Wake> = due.iter().filter_map(|key| clock.due.remove(key)).map(|(_, wake)| wake).collect();
    for wake in wakes {
        wake(cx);
    }
    // Wakes asked for again while running (a spinner's next step) armed the clock themselves.
    let clock = cx.global_mut::<FrameClock>();
    if let Some(next) = clock.due.values().map(|(at, _)| *at).min()
        && clock.armed.is_none_or(|armed| armed > next)
        && let Some(epoch) = clock.epoch
    {
        let now = cx.background_executor().now();
        arm(next, epoch, now, cx);
    }
}
