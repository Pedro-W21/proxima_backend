use std::{ffi::CStr, io::{Read, Write}, net::{Ipv4Addr, TcpListener, TcpStream}, sync::mpsc::{Receiver, Sender, channel}, thread, time::{Duration, Instant}};

use pyo3::{Python, pyclass, pymethods, types::PyAnyMethods};

pub fn logging_thread(mut stream:TcpStream, chunk_recv:Receiver<String>) {
    while let Ok(chunk) = chunk_recv.recv_timeout(Duration::from_millis(4000)) {
        stream.write_all(chunk.as_bytes()).unwrap();
    }
    stream.write(&[255]).unwrap();
}

#[pyclass]
struct LoggingStdio {
    chunk_send:Sender<String>,
    stdio_chunk:String,
    mode:String,
    last_sent:Instant
}

#[pymethods]
impl LoggingStdio {
    fn write(&mut self, data: &str) {
        self.stdio_chunk += data;
        // if self.last_sent.elapsed().as_millis() > 500 && self.stdio_chunk.chars().size_hint().0 > 0 {
            self.chunk_send.send(format!("{}\n{}", self.mode.clone(), self.stdio_chunk.clone())).unwrap();
            self.stdio_chunk.clear();
            self.last_sent = Instant::now();
        // }
    }
}

fn main() {
    let listener = TcpListener::bind((Ipv4Addr::new(127, 0, 0, 1), 4096)).unwrap();
    let mut stream = listener.accept().unwrap();
    let command = read_proxima_python_toolcall_string(&mut stream.0).unwrap();
    let lines:Vec<&str> = command.lines().collect();
    let (s, r) = channel();
    thread::spawn(move || {
        logging_thread(stream.0, r);
    });
    Python::attach(|py| {

        let sys = py.import("sys").unwrap();
        let logger_stdout = LoggingStdio {chunk_send:s.clone(), stdio_chunk:String::with_capacity(512), last_sent:Instant::now(), mode:String::from("stdout_output")};
        let logger_stderr = LoggingStdio {chunk_send:s, stdio_chunk:String::with_capacity(512), last_sent:Instant::now(), mode:String::from("stderr_output")};
        sys.setattr("stdout_prox", logger_stdout).unwrap();
        sys.setattr("stderr_prox", logger_stderr).unwrap();
        match lines[0] {
            "eval" => if lines.len() >= 2 {
                let mut final_expr = lines[1].trim().to_string();
                final_expr.push('\0');
                py.eval(CStr::from_bytes_until_nul(final_expr.as_bytes()).unwrap(), None, None).unwrap();
            },
            "run" => if lines.len() >= 2 {

                let mut final_program = lines.iter().skip(1).map(|line| {format!("{}\n", line)}).collect::<Vec<String>>().concat();
                final_program.push('\0');
                py.run(CStr::from_bytes_until_nul(final_program.as_bytes()).unwrap(), None, None).unwrap();
            },
            _ => ()
        }
    });
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
                else {
                    for i in 0..(read_bytes-1) {
                        bytes.push(reading_buffer[i]);
                    }
                }
            },
            Err(error) => return Err(()),
        }
    }
    
}