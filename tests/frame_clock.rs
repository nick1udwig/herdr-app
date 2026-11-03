use gpui::{AppContext as _, TestAppContext};
use gpui_shell::frame_clock::{TICK, wake, wake_at};
use std::{cell::Cell, rc::Rc, time::Duration};

#[gpui::test]
fn text_frame_deadlines_wake_once_per_change(cx: &mut TestAppContext) {
    let view = cx.new(|_| ());
    let epoch = cx.executor().now();
    let period = Duration::from_millis(90);
    let wakes = Rc::new(Cell::new(0));
    let mut frame = 0;
    for _ in 0..11 {
        let deadline = epoch + period * (frame + 1);
        let count = wakes.clone();
        cx.update(|cx| wake_at(view.entity_id(), deadline, cx, move |_| count.set(count.get() + 1)));
        cx.run_until_parked();
        assert!(cx.dispatcher.advance_clock_to_next_timer());
        cx.run_until_parked();
        let now = cx.executor().now();
        assert!(now >= deadline, "text frame woke before its next change");
        assert!(now - deadline < TICK, "text frame missed the next shared tick");
        let next_frame = (now.duration_since(epoch).as_millis() / period.as_millis()) as u32;
        assert_eq!(next_frame, frame + 1);
        frame = next_frame;
        assert_eq!(wakes.get(), frame);
    }
    assert!(!cx.dispatcher.advance_clock_to_next_timer());
}

#[gpui::test]
fn deadlines_share_ticks_and_keep_the_earliest_wake(cx: &mut TestAppContext) {
    let first = cx.new(|_| ());
    let second = cx.new(|_| ());
    let interval = cx.new(|_| ());
    let epoch = cx.executor().now();
    let wakes = Rc::new(Cell::new(0));
    for (view, after) in [(&first, 80), (&second, 90)] {
        let count = wakes.clone();
        cx.update(|cx| wake_at(view.entity_id(), epoch + Duration::from_millis(after), cx, move |_| count.set(count.get() + 1)));
    }
    cx.update(|cx| wake_at(first.entity_id(), epoch + Duration::from_millis(95), cx, |_| panic!("later wake replaced an earlier one")));
    cx.update(|cx| wake_at(second.entity_id(), epoch + Duration::from_millis(150), cx, |_| panic!("later deadline replaced an earlier one")));
    let count = wakes.clone();
    cx.update(|cx| wake(interval.entity_id(), Duration::from_millis(100), cx, move |_| count.set(count.get() + 1)));
    cx.run_until_parked();
    cx.executor().advance_clock(TICK * 3 - Duration::from_nanos(1));
    cx.run_until_parked();
    assert_eq!(wakes.get(), 0);
    cx.executor().advance_clock(Duration::from_nanos(1));
    cx.run_until_parked();
    assert_eq!(wakes.get(), 3);
    assert!(!cx.dispatcher.advance_clock_to_next_timer());
}
