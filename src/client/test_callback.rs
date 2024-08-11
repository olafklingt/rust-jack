use std::sync::atomic::Ordering;
use std::thread::sleep;
use std::time::Duration;
use std::{ptr, thread};

use super::*;
use crate::{test_tools, AudioIn, AudioOut, Client, Control, NotificationHandler, ProcessHandler};

#[test]
fn client_cback_has_proper_default_callbacks() {
    // defaults shouldn't care about these params
    let wc = unsafe { Client::from_raw(ptr::null_mut()) };
    let ps = unsafe { ProcessScope::from_raw(0, ptr::null_mut()) };
    // check each callbacks
    ().thread_init(&wc);
    ().shutdown(client_status::ClientStatus::empty(), "mock");
    assert_eq!(().process(&wc, &ps), Control::Continue);
    ().freewheel(&wc, true);
    ().freewheel(&wc, false);
    assert_eq!(().buffer_size(&wc, 0), Control::Continue);
    assert_eq!(().sample_rate(&wc, 0), Control::Continue);
    ().client_registration(&wc, "mock", true);
    ().client_registration(&wc, "mock", false);
    ().port_registration(&wc, 0, true);
    ().port_registration(&wc, 0, false);
    assert_eq!(
        ().port_rename(&wc, 0, "old_mock", "new_mock"),
        Control::Continue
    );
    ().ports_connected(&wc, 0, 1, true);
    ().ports_connected(&wc, 2, 3, false);
    assert_eq!(().graph_reorder(&wc), Control::Continue);
    assert_eq!(().xrun(&wc), Control::Continue);

    std::mem::forget(wc);
}

#[test]
fn client_cback_calls_thread_init() {
    let ac = test_tools::active_test_client("client_cback_calls_thread_init");
    let counter = ac.deactivate().unwrap().1;
    // IDK why this isn't 1, even with a single thread.
    assert!(counter.thread_init_count.load(Ordering::Relaxed) > 0);
}

#[test]
fn client_cback_calls_process() {
    let ac = test_tools::active_test_client("client_cback_calls_process");
    let counter = ac.deactivate().unwrap().2;
    assert!(counter.frames_processed > 0);
    assert!(counter.last_frame_time > 0);
    assert!(counter.frames_since_cycle_start > 0);
}

#[test]
fn client_cback_calls_buffer_size() {
    let ac = test_tools::active_test_client("client_cback_calls_buffer_size");
    let initial = ac.as_client().buffer_size();
    let second = initial / 2;
    let third = second / 2;
    ac.as_client().set_buffer_size(second).unwrap();
    sleep(Duration::from_millis(1));
    ac.as_client().set_buffer_size(third).unwrap();
    sleep(Duration::from_millis(1));
    ac.as_client().set_buffer_size(initial).unwrap();
    sleep(Duration::from_millis(1));
    let counter = ac.deactivate().unwrap().2;
    let mut history_iter = counter.buffer_size_change_history.iter().cloned();
    assert_eq!(history_iter.find(|&s| s == initial), Some(initial));
    assert_eq!(history_iter.find(|&s| s == second), Some(second));
    assert_eq!(history_iter.find(|&s| s == third), Some(third));
    assert_eq!(history_iter.find(|&s| s == initial), Some(initial));
}

/// Tests the assumption that the buffer_size callback is called on the process
/// thread. See issue #137
#[test]
#[ignore]
fn client_cback_calls_buffer_size_on_process_thread() {
    let ac = test_tools::active_test_client("cback_buffer_size_process_thr");
    let initial = ac.as_client().buffer_size();
    let second = initial / 2;
    ac.as_client().set_buffer_size(second).unwrap();
    let counter = ac.deactivate().unwrap().2;
    dbg!(&counter);
    let process_thread = counter.process_thread.unwrap();
    assert_eq!(
        counter.buffer_size_thread_history,
        [process_thread, process_thread],
        "Note: This does not hold for JACK2 and pipewire",
    );
}

#[test]
fn client_cback_calls_after_client_registered() {
    let ac = test_tools::active_test_client("client_cback_cacr");
    let _other_client = test_tools::open_test_client("client_cback_cacr_other");
    let counter = ac.deactivate().unwrap().1;
    assert!(counter
        .registered_client_history
        .contains(&"client_cback_cacr_other".to_string(),));
    assert!(!counter
        .unregistered_client_history
        .contains(&"client_cback_cacr_other".to_string(),));
}

#[test]
fn client_cback_calls_after_client_unregistered() {
    let ac = test_tools::active_test_client("client_cback_cacu");
    let other_client = test_tools::open_test_client("client_cback_cacu_other");
    drop(other_client);
    let counter = ac.deactivate().unwrap().1;
    assert!(counter
        .registered_client_history
        .contains(&"client_cback_cacu_other".to_string(),));
    assert!(counter
        .unregistered_client_history
        .contains(&"client_cback_cacu_other".to_string(),));
}

#[test]
#[ignore]
fn client_cback_reports_xruns() {
    let c = test_tools::open_test_client("client_cback_reports_xruns").0;
    let counter = test_tools::Counter {
        induce_xruns: true,
        ..test_tools::Counter::default()
    };
    let ac = c
        .activate_async(test_tools::Counter::default(), counter)
        .unwrap();
    let _pa = ac.as_client().register_port("i", AudioIn).unwrap();
    let _pa = ac.as_client().register_port("o", AudioOut).unwrap();
    thread::sleep(Duration::from_secs(5));

    let rtuple = ac.deactivate();
    dbg!(&rtuple);
    let (client, counter, counter2) = rtuple.unwrap();
    assert!(counter.xruns_count > 0, "No xruns encountered.");
    drop(counter);
    drop(counter2);
    drop(client);
}

#[test]
fn client_cback_calls_port_registered() {
    let ac = test_tools::active_test_client("client_cback_cpr");
    let _pa = ac.as_client().register_port("pa", AudioIn).unwrap();
    let _pb = ac.as_client().register_port("pb", AudioIn).unwrap();

    sleep(Duration::from_secs(10));
    let counter = ac.deactivate().unwrap().1;
    assert_eq!(
        counter.port_register_history.len(),
        2,
        "Did not detect port registrations."
    );
    assert!(
        counter.port_unregister_history.is_empty(),
        "Detected false port deregistrations."
    );
}

#[test]
fn client_cback_calls_port_unregistered() {
    let ac = test_tools::active_test_client("client_cback_cpr");
    let pa = ac.as_client().register_port("pa", AudioIn).unwrap();
    let pb = ac.as_client().register_port("pb", AudioIn).unwrap();
    ac.as_client().unregister_port(pa).unwrap();
    ac.as_client().unregister_port(pb).unwrap();
    let counter = ac.deactivate().unwrap().1;
    assert!(
        counter.port_register_history.len() >= 2,
        "Did not detect port registrations."
    );
    assert!(
        counter.port_unregister_history.len() >= 2,
        "Did not detect port deregistrations."
    );
}
