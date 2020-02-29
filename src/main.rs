use std::sync::mpsc;
use std::thread;

//type WorkData = f32;
const PORTS_SIZE: usize = 10;
type Port = [f32; PORTS_SIZE];
struct Ports {
    input: Port,
    output: Port,
}

enum Message {
    Job(Port),
    Terminate,
}

struct Host {
    htx: mpsc::Sender<Message>,
    hrx: mpsc::Receiver<Port>,
    //rx: mpsc::Receiver<f32>,
    run_handle: Option<thread::JoinHandle<()>>,
    //plugin: Plugin,
}

impl Host {
    fn new(plugin: Plugin) -> Self {
        let (htx, prx) = mpsc::channel::<Message>();
        let (ptx, hrx) = mpsc::channel::<Port>();
        let mut tmp = Self {
            htx,
            hrx,
            run_handle: None,
            //plugin,
        };
        let builder = thread::Builder::new().name(String::from("runner"));
        let run_handle = Some(
            builder
                .spawn(move || Self::run_loop(prx, ptx, plugin))
                .unwrap(),
        );
        tmp.run_handle = run_handle;
        tmp
    }

    fn send(&self, input: Port) {
        self.htx.send(Message::Job(input)).unwrap();
    }

    fn recv(&self) -> Port {
        self.hrx.recv().unwrap()
    }

    fn join(&mut self) {
        let _ = self.htx.send(Message::Terminate);
        println!("join");
        let _ = self.run_handle.take().unwrap().join();
        //if let Some(handle)= self.run_handle {
        //    self.run_handle = None;
        //    handle.join();
        //}
    }

    fn run_loop(rx: mpsc::Receiver<Message>, tx: mpsc::Sender<Port>, mut plugin: Plugin) {
        loop {
            let re = rx.recv().unwrap();
            match re {
                Message::Job(port) => {
                    let mut ports = Ports {
                        input: port,
                        output: [0f32; PORTS_SIZE],
                    };
                    plugin.run(&mut ports);
                    let _ = tx.send(ports.output);
                }
                Message::Terminate => break,
            }
        }
    }
}

struct Plugin {}

impl Plugin {
    fn run(&mut self, ports: &mut Ports) {
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
