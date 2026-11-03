use gpui::{IntoElement as _, TestAppContext};
use gpui_shell::{HostModule, HostValue, ShellRuntime, policy::Policy};
use std::{path::PathBuf, time::Duration};

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        gpui_shell::set_spinners(true);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn record(fields: &[(&str, &str)]) -> HostValue {
    HostValue::Object(fields.iter().map(|(key, value)| (key.to_string(), HostValue::Str(value.to_string()))).collect())
}

// A separate test process keeps the global spinner setting out of other tests.
#[gpui::test]
fn peek_spinner_stops_scheduling_redraws_when_disabled(cx: &mut TestAppContext) {
    cx.update(gpui_shell::init);
    let fixture = Fixture(std::env::temp_dir().join(format!("herdr-spinners-{}", std::process::id())));
    std::fs::create_dir_all(&fixture.0).unwrap();
    std::fs::write(fixture.0.join("main.js"), include_str!("../defaults/mods/peek/main.js")).unwrap();
    let mut module = HostModule::new("herdr");
    for (name, value) in [
        ("props", record(&[("terminal", "t1")])),
        ("theme", record(&[("surface", "#112233"), ("foreground", "#ffffff"), ("muted", "#888888"), ("dim", "#444444"), ("hover", "#223344"), ("border", "#334455"), ("working", "#abcdef")])),
        ("terminals", HostValue::Array(vec![record(&[("address", "t1"), ("status", "working")])])),
        ("agents", HostValue::Array(vec![])),
        ("panes", HostValue::Array(vec![])),
    ] {
        module = module.function(name, move |_| Ok(value.clone()));
    }
    for name in ["run", "closeOverlay"] {
        module = module.function(name, move |_| panic!("unexpected {name}"));
    }
    gpui_shell::policy::set_default(Policy::new().with_host_module(module).unwrap());
    let runtime = ShellRuntime::new_isolated().unwrap();
    let context = cx.add_empty_window();
    let root = context.update(|window, cx| runtime.try_load(&fixture.0, window, cx).unwrap());
    let view = root.read_with(context, |root, _| root.content().clone());

    for turning in [false, true, false, true] {
        gpui_shell::set_spinners(turning);
        context.executor().advance_clock(Duration::from_secs(1));
        context.run_until_parked();
        context.update(|_, cx| cx.notify(view.entity_id()));
        context.draw(gpui::Point::default(), gpui::size(gpui::px(600.), gpui::px(40.)), |_, _| view.clone().into_any_element());
        context.run_until_parked();
        assert_eq!(context.dispatcher.advance_clock_to_next_timer(), turning, "peek spinner timer with spinners={turning}");
    }
    context.update(|window, _| window.remove_window());
}
