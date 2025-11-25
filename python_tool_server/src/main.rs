use std::{env, io::{Read, Write}, net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream}, process::Command, sync::{Arc, atomic::{AtomicUsize, Ordering}, mpsc::{Receiver, RecvTimeoutError, Sender, channel}}, thread, time::{Duration, Instant}};

fn main() {
    let (s, r) = channel();

    let args:Vec<String> = env::args().collect();
    let mut python_port = 4096;
    let mut start_docker_ports = 4100;
    let mut max_in_flight = 4;
    if args.len() == 4 {
        python_port = args[1].parse().expect("First argument is supposed to be a valid port number if it is there");
        start_docker_ports = args[2].parse().expect("Second argument is supposed to be a valid port number if the first is there");
        max_in_flight = args[3].parse().expect("Third argument is supposed to be a max number of parallel python calls if the first is there");
    }
    let mut listener = ListenerData::new(python_port, s);
    let mut queue_handler = QueueHandlerData::new(r, max_in_flight, start_docker_ports);
    thread::spawn(move || {
        queue_handler.handling_loop();
    });
    listener.listening_loop();
}

pub enum PythonToolRequest {
    Eval(String),
    Run(String)
}

impl PythonToolRequest {
    pub fn to_string(&self) -> String {
        match self {
            PythonToolRequest::Run(program) => format!("run\n{program}"),
            PythonToolRequest::Eval(expression) => format!("eval\n{expression}")
        }
    }
}

pub struct ListenerData {
    listener:TcpListener,
    queue:Sender<((TcpStream, SocketAddr), PythonToolRequest)>
}

pub struct QueueHandlerData {
    queue:Receiver<((TcpStream, SocketAddr), PythonToolRequest)>,
    in_flight:usize,
    start_port:u16,
    max_in_flight:usize,
    thread_counter:usize,
    ports:Vec<u16>,
    finished_queue:(Sender<u16>, Receiver<u16>)
}

pub struct InFlightExecution {
    proxima_stream:(TcpStream, SocketAddr),
    executor_stream:(TcpStream, SocketAddr),
    start_time:Instant,
    thread_number:usize,
    id:String,
    finish_sender:Sender<u16>
}

impl InFlightExecution {
    pub fn new(mut proxima_stream:(TcpStream, SocketAddr), request:PythonToolRequest, thread_number:usize, port:u16, finish_sender:Sender<u16>) -> Result<Self, ()> {
        println!("Creating execution");
        let command = Command::new("docker").args(vec!["run", "-d"/*, "--name", &format!("proxima_python_{}", thread_number)*/, "-p", &format!("127.0.0.1:{port}:4096/tcp"), "proxima_python_executor:any" ]).output();
        match command {
            Ok(output) => match String::from_utf8(output.stdout) {
                Ok(id) => {

                    println!("Trying to connect to executor");
                    let addr = SocketAddr::from((Ipv4Addr::new(127, 0, 0, 1), port));
                    match TcpStream::connect_timeout(&addr, Duration::from_millis(5000)) {
                        Ok(mut python_stream) => {
                            println!("Connected to executor");

                            println!("Sending request :\n{}", request.to_string());
                            python_stream.set_read_timeout(Some(Duration::from_millis(10000))).unwrap();
                            python_stream.set_write_timeout(Some(Duration::from_millis(1000))).unwrap();
                            write_proxima_string_to_stream(&mut python_stream, request.to_string());
                            return Ok(InFlightExecution { proxima_stream, executor_stream: (python_stream, addr), start_time: Instant::now(), thread_number, finish_sender,id })
                        },
                        Err(error) => {
                            write_proxima_string_to_stream(&mut proxima_stream.0, format!("Couldn't reach the executor : {}", error));
                            finish_sender.send(port).unwrap()
                        },
                    }
                },
                Err(error) => {
                    write_proxima_string_to_stream(&mut proxima_stream.0, format!("Couldn't read the container creation output : {}", error));
                    finish_sender.send(port).unwrap()
                },
            },
            Err(error) => {
                write_proxima_string_to_stream(&mut proxima_stream.0, format!("Couldn't run the container creation command : {}", error));
                finish_sender.send(port).unwrap()
            },
        }
        Err(())
    }
    pub fn execute(&mut self) {
        const DEFAULT_PYTHON_TIMEOUT:u128 = 15000;
        while self.start_time.elapsed().as_millis() < DEFAULT_PYTHON_TIMEOUT {
            let mut buf = vec![0 ; 4096];
            match self.executor_stream.0.read(&mut buf) {
                Ok(bytes_read) => match self.proxima_stream.0.write(&buf[..bytes_read]) {
                    Ok(_) => println!("Passed along {} bytes", bytes_read),
                    Err(error) => {
                        println!("Error during proxima write : {}", error);
                        break
                    }
                },
                Err(error) => {
                    println!("Error during execution read : {}", error);
                    break
                },
            }
            thread::sleep(Duration::from_millis(100));
        }
        self.finish();
    }
    pub fn finish(&mut self) {
        println!("Finished execution");
        match self.proxima_stream.0.write(&[255]) {
            Ok(written) => (),
            Err(_) => ()
        }
        Command::new("docker").args(vec!["kill", &self.id]).output();
        self.finish_sender.send(self.executor_stream.1.port()).unwrap();
    }
}

impl QueueHandlerData {
    pub fn new(queue:Receiver<((TcpStream, SocketAddr), PythonToolRequest)>, max_in_flight:usize, start_port:u16) -> Self {
        let mut ports = Vec::with_capacity(max_in_flight);
        for port in start_port..(start_port + max_in_flight as u16) {
            ports.push(port);
        }
        Self { queue, in_flight:0, max_in_flight, ports, start_port, finished_queue:channel(), thread_counter:0 }
    }
    pub fn handling_loop(&mut self) {
        loop {
            if self.in_flight < self.max_in_flight && self.ports.len() > 0 {
                match self.queue.recv() {
                    Ok((proxima_stream, request)) => {
                        let port = self.ports.remove(0);
                        let finish_sender = self.finished_queue.0.clone();
                        self.in_flight += 1;
                        let count = self.thread_counter;
                        thread::spawn(move || {
                            match InFlightExecution::new(proxima_stream, request, count, port, finish_sender) {
                                Ok(mut exec) => exec.execute(),
                                Err(_) => ()
                            }
                        });
                        self.thread_counter += 1;

                    },
                    Err(_) => todo!("handle error"),
                }
                match self.finished_queue.1.recv_timeout(Duration::from_millis(1000)) {
                    Ok(port) => {
                        self.in_flight -= 1;
                        self.ports.push(port);
                    },
                    Err(error) => match error {
                        RecvTimeoutError::Timeout => continue,
                        RecvTimeoutError::Disconnected => todo!("Handle error")
                    },
                }
            }
            else {
                match self.finished_queue.1.recv() {
                    Ok(port) => {
                        self.in_flight -= 1;
                        self.ports.push(port);
                    },
                    Err(error) => todo!("Handle this error")
                }
            }
        }
    }
}

pub fn write_proxima_string_to_stream(stream:&mut TcpStream, message_string:String) {
    let mut message = message_string.as_bytes().iter().map(|utf8| {*utf8}).collect::<Vec<u8>>();
    message.push(255);
    stream.write_all(&message).unwrap();
}

fn read_proxima_python_toolcall_string(stream:&mut TcpStream) -> Result<String, ()> {
    let mut bytes = Vec::with_capacity(1024);
    let mut reading_buffer = vec![0 ; 1500];
    loop {
        match stream.read(&mut reading_buffer) {
            Ok(read_bytes) => {
                
                if reading_buffer[..read_bytes].contains(&255) {
                    if read_bytes > 1 {
                        for i in 0..(read_bytes-1) {
                            bytes.push(reading_buffer[i]);
                        }
                    }
                    match String::from_utf8(bytes) {
                        Ok(string) => return Ok(string),
                        Err(error) => return Err(()),
                    }
                }
                else if read_bytes > 0 {
                    for i in 0..read_bytes {
                        bytes.push(reading_buffer[i]);
                    }
                }
            },
            Err(error) => return Err(()),
        }
    }
    
}

impl ListenerData {
    pub fn new(port:u16, queue:Sender<((TcpStream, SocketAddr), PythonToolRequest)>) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::new(127, 0, 0, 1), port)).unwrap();
        Self { listener, queue }
    }
    pub fn listening_loop(&mut self) {
        while let Ok(mut stream) = self.listener.accept() {
            match read_proxima_python_toolcall_string(&mut stream.0) {
                Ok(input) => {
                    let lines:Vec<&str> = input.lines().collect();
                    if lines.len() > 1 {   
                        let data = input.trim_start_matches(lines[0]).trim();
                        match lines[0] {
                            "run" => {
                                self.queue.send((stream, PythonToolRequest::Run(data.to_string()))).unwrap();
                            },
                            "eval" => {
                                self.queue.send((stream, PythonToolRequest::Eval(data.to_string()))).unwrap();
                            },
                            _ => {write_proxima_string_to_stream(&mut stream.0,format!("Invalid format"));}
                        }
                    }
                    else {
                        write_proxima_string_to_stream(&mut stream.0,format!("Invalid number of arguments"));
                    }
                },
                Err(_) => {write_proxima_string_to_stream(&mut stream.0,format!("Invalid command format"));}
            }
        }
    }
}

