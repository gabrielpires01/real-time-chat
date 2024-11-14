use std::net::{Shutdown, TcpListener, TcpStream};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::collections::HashMap;

enum Messages {
    New(Vec<u8>, String),
    Disconnect(String),
    Connect(TcpStream, String),
    Private(Vec<u8>, String, String),
}

const BLUE: &str = "\x1b[34m";
const RESET: &str = "\x1b[0m";

fn main() {
    let address = "127.0.0.1:7777";

    let listener = TcpListener::bind(address).unwrap_or_else(|error| {
        println!("{:?}", error);
        panic!("Could no bind to {}", &address)
    });
    
    let (sender, receiver): (Sender<Messages> , Receiver<Messages>) = channel();

    let users: Arc<Mutex<HashMap<String, TcpStream>>> = Arc::new(Mutex::new(HashMap::new()));

    {
        let users_clone = Arc::clone(&users);
        thread::spawn(move || server(receiver, users_clone));
    }
    
    for stream in listener.incoming() {
        let users_clone = Arc::clone(&users);
        match stream {
            Ok(stream) => {
                let sender = sender.clone();
                thread::spawn(|| client(sender, stream, users_clone));
            }
            Err(_e) => {
                println!("Could not read stream")
            }
        };
    };
}

fn server(receiver: Receiver<Messages>, users: Arc<Mutex<HashMap<String, TcpStream>>>) {
    loop {
        let messages = receiver.recv().unwrap();
        match messages {
            Messages::New(values, user) => {
                println!("pq?");
                let users_map = users.lock().unwrap();
                for (address, mut stream) in users_map.iter() {
                    if *address == user {
                        continue;
                    }
                    
                    let user_intro = format!("{}: ", user.trim()).as_bytes().to_vec(); 
                    let val = [user_intro, values.clone()].concat();
                    stream.write(&val).unwrap();
                }
            }
            Messages::Disconnect(address) => {
                println!("Disconnected");
                let mut users_map = users.lock().unwrap();
                users_map.remove(&address); 
            }
            Messages::Connect(stream, user) => {
                println!("Connection established with {}", user);
                let mut users_map = users.lock().unwrap();
                users_map.insert(user, stream);
            }
            Messages::Private(values, from_user, to_user) => {
                let users_map = users.lock().unwrap();
                let mut to_stream = users_map.get(&to_user).unwrap();
                let private_user = format!("{}Private message from {}: {}", BLUE, from_user.trim(), RESET).as_bytes().to_vec(); 
                let private_val = [private_user, values.clone()].concat();
                to_stream.write(&private_val).unwrap();
            }
        }
    }
}

fn client(sender: Sender<Messages>, mut stream: TcpStream, users: Arc<Mutex<HashMap<String, TcpStream>>>)  {
    let mut buffer = Vec::new();
    buffer.resize(64, 0);

    let user = loop {
        stream.write(b"Seu nome: ").unwrap();
        let read_from_stream = stream.read(&mut buffer);
        match read_from_stream {
            Ok(n) => {
                let buffer_slice = &buffer[0..n];
                let user_intro = b"Changing username to ".to_vec(); 
                let val = [user_intro, buffer_slice.to_vec().clone()].concat();
                stream.write(&val).unwrap();
                let users_map = users.lock().unwrap();
                match String::from_utf8(buffer_slice.to_vec()) {
                    Ok(username) => {
                        match users_map.get(&username) {
                            Some(_value) => {
                                stream.write(b"Name already taken\n").unwrap();
                            }
                            None => {
                                break username.replace("\r\n", "");
                            }
                        };
                    }
                    Err(_e) => {
                        todo!()
                    }
                }
            }
            Err(..) => {
                println!("No name inputed");
                stream.shutdown(Shutdown::Both).expect("shutdown connection");
            }
        }
    };
    sender.send(Messages::Connect(stream.try_clone().unwrap(), user.clone())).unwrap();
    loop {
        let cloned_user = user.clone();
        let read_from_stream = stream.read(&mut buffer);
        match read_from_stream {
            Ok(n) => {
                if n == 0 {
                    sender.send(Messages::Disconnect(cloned_user)).unwrap();
                    break
                };
                let buffer_slice = &buffer[0..n];
                match String::from_utf8(buffer_slice.to_vec()) {
                    Ok(message) => {
                        if message.starts_with("/p") {
                            let message = message.replace("/p ", "");
                            let split_message = message.split(" ").collect::<Vec<&str>>();
                            if split_message.len() < 2 {
                                stream.write(b"Invalid message. eg: /p user message\n").unwrap();
                                return
                            }
                            let to_user = split_message[0].trim().to_string();
                            sender.send(Messages::Private(split_message[1..].join(" ").as_bytes().to_vec(), cloned_user.clone(), to_user)).unwrap();
                        }else {
                            sender.send(Messages::New(buffer_slice.to_vec(), cloned_user.clone())).unwrap();
                        }
                    }
                    Err(_e) => {
                        todo!()
                    }
                }
            }
            Err(..) => {
                println!("Disconnected");
                stream.shutdown(Shutdown::Both).expect("shutdown connection");
                break
            }
        }
    }
}
