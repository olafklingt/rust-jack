use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Mutex};
use std::thread::{self};
use std::time::{self};

use crate::{client::*, AudioIn, Frames, PortId};
use crate::{Control, Port};

#[derive(Debug, Default)]
pub struct Counter {
    pub process_return_val: Control,
    pub induce_xruns: bool,
    pub thread_init_count: AtomicUsize,
    pub frames_processed: usize,
    pub process_thread: Option<thread::ThreadId>,
    pub buffer_size_thread_history: Vec<thread::ThreadId>,
    pub buffer_size_change_history: Vec<Frames>,
    pub registered_client_history: Vec<String>,
    pub unregistered_client_history: Vec<String>,
    pub port_register_history: Vec<PortId>,
    pub port_unregister_history: Vec<PortId>,
    pub xruns_count: usize,
    pub last_frame_time: Frames,
    pub frames_since_cycle_start: Frames,
}

impl NotificationHandler for Counter {
    fn thread_init(&self, _: &Client) {
        self.thread_init_count.fetch_add(1, Ordering::Relaxed);
    }

    fn client_registration(&mut self, _: &Client, name: &str, is_registered: bool) {
        if is_registered {
            self.registered_client_history.push(name.to_string())
        } else {
            self.unregistered_client_history.push(name.to_string())
        }
    }

    fn port_registration(&mut self, _: &Client, pid: PortId, is_registered: bool) {
        if is_registered {
            self.port_register_history.push(pid)
        } else {
            self.port_unregister_history.push(pid)
        }
    }

    fn xrun(&mut self, _: &Client) -> Control {
        dbg!(self.xruns_count);
        self.xruns_count += 1;
        Control::Continue
    }
}

impl ProcessHandler for Counter {
    fn process(&mut self, _: &Client, ps: &ProcessScope) -> Control {
        self.frames_processed += ps.n_frames() as usize;
        self.last_frame_time = ps.last_frame_time();
        self.frames_since_cycle_start = ps.frames_since_cycle_start();
        let _cycle_times = ps.cycle_times();
        if self.induce_xruns {
            thread::sleep(time::Duration::from_millis(1000));
        }
        self.process_thread = Some(thread::current().id());
        Control::Continue
    }

    fn buffer_size(&mut self, _: &Client, size: Frames) -> Control {
        self.buffer_size_change_history.push(size);
        self.buffer_size_thread_history.push(thread::current().id());
        Control::Continue
    }
}

pub struct PortIdHandler {
    pub reg_tx: Mutex<mpsc::SyncSender<PortId>>,
}

impl NotificationHandler for PortIdHandler {
    fn port_registration(&mut self, _: &Client, pid: PortId, is_registered: bool) {
        if is_registered {
            self.reg_tx.lock().unwrap().send(pid).unwrap()
        }
    }
}

pub fn open_test_client(name: &str) -> (Client, ClientStatus) {
    Client::new(name, ClientOptions::NO_START_SERVER).unwrap()
}

pub fn active_test_client(name: &str) -> AsyncClient<Counter, Counter> {
    let c = open_test_client(name).0;
    c.activate_async(Counter::default(), Counter::default())
        .unwrap()
}

pub fn open_client_with_port(
    client: &str,
    port: &str,
) -> (AsyncClient<Counter, Counter>, Port<AudioIn>) {
    let c = open_test_client(client).0;
    let ac = c
        .activate_async(Counter::default(), Counter::default())
        .unwrap();
    let p = ac.as_client().register_port(port, AudioIn).unwrap();
    (ac, p)
}
