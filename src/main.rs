use std::net::{Shutdown, TcpListener, TcpStream};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::collections::HashMap;

enum Messages {
    New(Vec<u8>, String),
    Disconnect(String),
    Connect(String, TcpStream),
}

fn main() {
    let address = "127.0.0.1:7777";

    let listener = TcpListener::bind(address).unwrap_or_else(|error| {
        println!("{:?}", error);
        panic!("Could no bind to {}", &address)
    });
    
    let (sender, receiver): (Sender<Messages> , Receiver<Messages>) = channel();

    let users: HashMap<String, TcpStream> = HashMap::new();

    thread::spawn(move || server(receiver, users));
    
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let sender = sender.clone();
                let author_address = stream.peer_addr().unwrap();
                let user = author_address.to_string();
                sender.send(Messages::Connect(user, stream.try_clone().unwrap())).unwrap();
                thread::spawn(|| client(sender, stream));
            }
            Err(_e) => {
                panic!("Could not read stream")
            }
        };
    };
}


fn server(receiver: Receiver<Messages>, mut users: HashMap<String, TcpStream>) {
    loop {
        let messages = receiver.recv().unwrap();
        match messages {
            Messages::New(values, user) => {
                for (address, mut stream) in &users {
                    if *address == user {
                        continue;
                    }

                    stream.write(&values).unwrap();
                }
            }
            Messages::Disconnect(address) => {
                println!("Disconnected");
                users.remove(&address); 
            }
            Messages::Connect(address, stream) => {
                println!("Connection established with {}", address);
                users.insert(address, stream);
            }
        }
    }
}

fn client(sender: Sender<Messages>, mut stream: TcpStream)  {
    let mut buffer = Vec::new();
    buffer.resize(64, 0);
    loop {
        let read_from_stream = stream.read(&mut buffer);
        match read_from_stream {
            Ok(n) => {
                let author_address = stream.peer_addr().unwrap();
                let user = author_address.to_string();
                if n == 0 {
                    sender.send(Messages::Disconnect(user)).unwrap();
                    break
                };
                let buffer_slice = &buffer[0..n];
                sender.send(Messages::New(buffer_slice.to_vec(), user)).unwrap();
            }
            Err(..) => {
                println!("Disconnected");
                stream.shutdown(Shutdown::Both).expect("shutdown connection");
                break
            }
        }
    }
}
