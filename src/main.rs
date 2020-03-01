use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

type WorkData = f32;
const PORTS_SIZE: usize = 10;
type Port = [f32; PORTS_SIZE];
struct Ports {
    input: Port,
    output: Port,
}

enum RunMessage {
    Job(Port),
    Terminate,
}

enum WorkMessage {
    Job(WorkData),
    Terminate,
}

struct Host {
    run_htx: Option<mpsc::Sender<RunMessage>>,
    run_hrx: Option<mpsc::Receiver<Port>>,
    run_handle: Option<thread::JoinHandle<()>>,
    work_htx: Option<Arc<Mutex<mpsc::Sender<WorkMessage>>>>,
    work_hrx: Option<mpsc::Receiver<WorkData>>,
    //rx: mpsc::Receiver<f32>,
    work_handle: Option<thread::JoinHandle<()>>,
    plugin: Option<Arc<Plugin>>,
}

impl Host {
    fn new() -> Self {
        Self {
            run_htx: None,
            run_hrx: None,
            run_handle: None,
            work_htx: None,
            work_hrx: None,
            work_handle: None,
            plugin: None,
        }
    }

    fn instanciate_plugin(&mut self,plug_factory: fn(features: Features)-> Option<Plugin>) {
        //build channel communitcation
        let (run_htx, run_prx) = mpsc::channel::<RunMessage>();
        let (run_ptx, run_hrx) = mpsc::channel::<Port>();
        let (work_htx, work_prx) = mpsc::channel::<WorkMessage>();
        let (work_ptx, work_hrx) = mpsc::channel::<WorkData>();
        let work_htx = Arc::new(Mutex::new(work_htx));
        let work_htx_ref2 = work_htx.clone();
        self.run_htx = Some(run_htx);
        self.run_hrx = Some(run_hrx);
        self.work_htx = Some(work_htx);
        self.work_hrx = Some(work_hrx);
        //schedule handler in features
        let schedule_handler = ScheduleHandler{ tx:work_htx_ref2};
        let features = Features{ schedule_handler: Some(schedule_handler)};
        //intanciate a plugin
        let plugin = if let Some(plugin) = (plug_factory)(features) {
            plugin
        } else {
            println!("Can't instanciate plugin");
            return;
        };
        let plugin_ref1 = Arc::new(plugin);
        let plugin_ref2 = Arc::clone(&plugin_ref1);
        let run_builder = thread::Builder::new().name(String::from("runner"));
        let run_handle = Some(
            run_builder
                .spawn(move || Self::run_loop(run_prx, run_ptx, plugin_ref1))
                .unwrap(),
        );
        self.run_handle = run_handle;

        let work_builder = thread::Builder::new().name(String::from("worker"));
        let work_handle = Some(
            work_builder
                .spawn(move || Self::work_loop(work_prx, work_ptx, plugin_ref2))
                .unwrap(),
        );
        self.work_handle = work_handle;
    }

    fn send(&mut self, input: Port) {
        let htx = self.run_htx.take().unwrap();
        htx.send(RunMessage::Job(input)).unwrap();
        self.run_htx = Some(htx);
    }

    fn recv(&mut self) -> Port {
        let hrx = self.run_hrx.take().unwrap();
        let res = hrx.recv().unwrap();
        self.run_hrx = Some(hrx);
        res
    }

    fn join(&mut self) {
        self.run_htx.take().unwrap().send(RunMessage::Terminate).unwrap();
        self.work_htx.take().unwrap().lock().unwrap().send(WorkMessage::Terminate).unwrap();
        println!("join");
        let _ = self.run_handle.take().unwrap().join();
        //if let Some(handle)= self.run_handle {
        //    self.run_handle = None;
        //    handle.join();
        //}
    }

    fn run_loop(rx: mpsc::Receiver<RunMessage>, tx: mpsc::Sender<Port>, plugin: Arc<Plugin>) {
        loop {
            let re = rx.recv().unwrap();
            match re {
                RunMessage::Job(port) => {
                    let mut ports = Ports {
                        input: port,
                        output: [0f32; PORTS_SIZE],
                    };
                    plugin.run(&mut ports);
                    let _ = tx.send(ports.output);
                }
                RunMessage::Terminate => {
                    println!("run_loop terminate");
                    break;
                }
            }
        }
    }

    fn work_loop(
        rx: mpsc::Receiver<WorkMessage>,
        _tx: mpsc::Sender<WorkData>,
        plugin: Arc<Plugin>,
    ) {
        loop {
            let re = rx.recv().unwrap();
            match re {
                WorkMessage::Job(work_data) => {
                    plugin.work(work_data);
                }
                WorkMessage::Terminate => {
                    println!("work_loop terminate");
                    break;
                }
            }
        }
    }
}

//feature to schedule work
struct ScheduleHandler {
    tx: Arc<Mutex<mpsc::Sender<WorkMessage>>>,
}

impl ScheduleHandler {
    fn schedule_work(& self, work_data: WorkData) {
        let _ = self.tx.lock().unwrap().send(WorkMessage::Job(work_data));
    }
}

//features
struct Features {
    schedule_handler: Option<ScheduleHandler>,
}

struct Plugin {
    schedule_handler: ScheduleHandler,
}

impl Plugin {
    fn new(features: Features) -> Option<Self> {
        let schedule_handler = if let Some(val) = features.schedule_handler {
            val
        } else {
            return None;
        };
        Some(Self { schedule_handler })
    }

    fn run(&self, ports: &mut Ports) {
        for (in_frame, out_frame) in Iterator::zip(ports.input.iter(), ports.output.iter_mut()) {
            *out_frame = in_frame * 0.5;
        }
        println!("run {:?}", &ports.input[0..10]);
        println!("schedule work");
        self.schedule_handler.schedule_work(12.34);
    }

    fn work(&self, data: WorkData) {
        println!("worker thread: {:?}", data);
    }
}

fn main() {
    let mut host = Host::new();
    host.instanciate_plugin(Plugin::new);

    for i in 0..10 {
        println!("send {}", i);
        host.send([i as f32; PORTS_SIZE]);
    }
    for i in 10..40 {
        println!("send {}", i);
        host.send([i as f32; PORTS_SIZE]);
        println!("recv {}", host.recv()[0]);
    }
    for _i in 40..50 {
        println!("recv {}", host.recv()[0]);
    }
    host.join();
}
