use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;
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

// Command to control Work thread
enum WorkControl {
    Job,
    Terminate,
}


//feature to schedule work
struct ScheduleHandler {
    ctl_tx: Arc<Mutex<mpsc::Sender<WorkControl>>>,
    data_tx: Mutex<mpsc::Sender<WorkData>>,
}

impl ScheduleHandler {
    // For real implementation, i think particular care needed here for reliability
    fn schedule_work(& self, work_data: WorkData) {
        // this part may require some compiler hint to avoid some reordering
        let _ = self.data_tx.try_lock().unwrap().send(work_data);
        let _ = self.ctl_tx.try_lock().unwrap().send(WorkControl::Job);
    }
}

struct Host {
    main_to_run_tx: Option<mpsc::Sender<Port>>,
    run_to_main_rx: Option<mpsc::Receiver<Port>>,
    run_handle: Option<thread::JoinHandle<()>>,
    work_ctl_tx: Option<Arc<Mutex<mpsc::Sender<WorkControl>>>>,
    work_handle: Option<thread::JoinHandle<()>>,
}

impl Host {
    fn new() -> Self {
        Self {
            main_to_run_tx: None,
            run_to_main_rx: None,
            run_handle: None,
            work_ctl_tx: None,
            work_handle: None,
        }
    }

    fn instanciate_plugin(&mut self,plug_factory: fn(features: Features)-> Option<Plugin>) {
        //build channel communitcation
        let (main_to_run_tx, main_to_run_rx) = mpsc::channel::<Port>();// main to run thread channel
        let (run_to_main_tx, run_to_main_rx) = mpsc::channel::<Port>();// run thread to main channel
        let (work_ctl_tx, work_ctl_rx) = mpsc::channel::<WorkControl>();
        let (run_to_work_tx, run_to_work_rx) = mpsc::channel::<WorkData>();
        self.main_to_run_tx = Some(main_to_run_tx);
        self.run_to_main_rx = Some(run_to_main_rx);
        let work_ctl_tx = Arc::new(Mutex::new(work_ctl_tx));
        self.work_ctl_tx = Some(Arc::clone(&work_ctl_tx));
        //schedule handler in features
        let schedule_handler = ScheduleHandler{ 
            ctl_tx: work_ctl_tx,
            data_tx:Mutex::new(run_to_work_tx),
        };
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

        // Build and start the run thread
        let run_builder = thread::Builder::new().name(String::from("runner"));
        self.run_handle = Some(
            run_builder
                .spawn(move || Self::run_loop(main_to_run_rx, run_to_main_tx, plugin_ref1))
                .unwrap(),
        );

        // Build and start the work thread
        let work_builder = thread::Builder::new().name(String::from("worker"));
        self.work_handle = Some(
            work_builder
                .spawn(move || Self::work_loop(work_ctl_rx, run_to_work_rx, plugin_ref2))
                .unwrap(),
        );
    }

    fn send(&mut self, input: Port) {
        let htx = self.main_to_run_tx.take().unwrap();
        htx.send(input).unwrap();
        self.main_to_run_tx = Some(htx);
    }

    fn recv(&mut self) -> Port {
        let hrx = self.run_to_main_rx.take().unwrap();
        let res = hrx.recv().unwrap();
        self.run_to_main_rx = Some(hrx);
        res
    }

    fn join(&mut self) {
        //Drop the send part of the channel to stop run loop
        {
            self.main_to_run_tx.take().unwrap();
        }
        self.work_ctl_tx.take().unwrap().lock().unwrap().send(WorkControl::Terminate).unwrap();
        println!("join");
        let _ = self.run_handle.take().unwrap().join();
        let _ = self.work_handle.take().unwrap().join();
    }

    // run context from the host side
    fn run_loop(rx: mpsc::Receiver<Port>, tx: mpsc::Sender<Port>, plugin: Arc<Plugin>) {
        // the while loop breaks when there is no data and when the send side have been dropped
        while let Ok(input_port) = rx.recv() {
            let mut ports = Ports {
                input: input_port,
                output: [0f32; PORTS_SIZE],
            };
            plugin.run(&mut ports);
            let _ = tx.send(ports.output);
        }
        println!("run_loop terminate, channel destroyed");
    }

    // work context from the host side
    fn work_loop(
        work_ctl_rx: mpsc::Receiver<WorkControl>,
        run_to_work_rx: mpsc::Receiver<WorkData>,
        plugin: Arc<Plugin>,
    ) {
        while let Ok(ctl) = work_ctl_rx.recv() {
            match ctl {
                WorkControl::Job => {
                    match run_to_work_rx.try_recv() {
                        Ok(data) => {plugin.work(data);}
                        Err(error) => {
                            match error {
                                TryRecvError::Empty => {
                                    panic!("Error, can't get data");
                                }
                                TryRecvError::Disconnected => {
                                    println!("work_loop terminate, channel disconnected");
                                    break;
                                }
                            }
                        }
                    }
                }
                WorkControl::Terminate => {
                    println!("work_loop terminate");
                    break;
                }
            }
        }
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
