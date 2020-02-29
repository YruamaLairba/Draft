use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

//type WorkData = f32;
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

struct Host {
    htx: Option<mpsc::Sender<RunMessage>>,
    hrx: Option<mpsc::Receiver<Port>>,
    //rx: mpsc::Receiver<f32>,
    run_handle: Option<thread::JoinHandle<()>>,
    //work_handle: Option<thread::JoinHandle<()>>,
    plugin: Arc<Plugin>,
}

impl Host {
    fn new(plugin: Plugin) -> Self {
        Self {
            htx: None,
            hrx: None,
            run_handle: None,
            //work_handle: None,
            plugin: Arc::new(plugin),
        }
    }

    fn start(&mut self) {
        let (htx, prx) = mpsc::channel::<RunMessage>();
        let (ptx, hrx) = mpsc::channel::<Port>();

        let plugin = Arc::clone(&self.plugin);
        let run_builder = thread::Builder::new().name(String::from("runner"));
        let run_handle = Some(
            run_builder
                .spawn(move || Self::run_loop(prx, ptx, plugin))
                .unwrap(),
        );
        self.htx = Some(htx);
        self.hrx = Some(hrx);
        self.run_handle = run_handle;
    }



    fn send(&mut self, input: Port) {
        let htx = self.htx.take().unwrap();
        htx.send(RunMessage::Job(input)).unwrap();
        self.htx = Some(htx);
    }

    fn recv(&mut self) -> Port {
        let hrx = self.hrx.take().unwrap();
        let res = hrx.recv().unwrap();
        self.hrx = Some(hrx);
        res
    }

    fn join(&mut self) {
        let htx = self.htx.take().unwrap();
        htx.send(RunMessage::Terminate).unwrap();
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
                RunMessage::Terminate => break,
            }
        }
    }
}

struct Plugin {}

impl Plugin {
    fn run(& self, ports: &mut Ports) {
        for (in_frame, out_frame) in Iterator::zip(ports.input.iter(), ports.output.iter_mut()) {
            *out_frame = in_frame * 0.5;
        }
        println!("{:?}", &ports.input[0..10]);
    }

    //fn work(&mut self, data: WorkData) {
    //    println!("worker thread: {:?}", data);
    //}
}

fn main() {
    let mut host = Host::new(Plugin {});
    host.start();

    for i in 0..10 {
        println!("send {}",i);
        host.send([i as f32;PORTS_SIZE]);
    }
    for i in 10..40 {
        println!("send {}", i);
        host.send([i as f32; PORTS_SIZE]);
        println!("recv {}", host.recv()[0]);
    }
    for _i in 40..50{
        println!("recv {}",host.recv()[0]);
    }
    host.join();
}
