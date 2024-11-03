use std::net::{Shutdown, TcpListener, TcpStream};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::collections::HashMap;

enum Messages {
    New(Vec<u8>, String),
    Disconnect(String),
    Connect(TcpStream, String),
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
                thread::spawn(|| client(sender, stream));
            }
            Err(_e) => {
                println!("Could not read stream")
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
                    
                    let user_intro = format!("{}: ", user.trim()).as_bytes().to_vec(); 
                    let val = [user_intro, values.clone()].concat();
                    stream.write(&val).unwrap();
                }
            }
            Messages::Disconnect(address) => {
                println!("Disconnected");
                users.remove(&address); 
            }
            Messages::Connect(stream, user) => {
                println!("Connection established with {}", user);
                users.insert(user, stream);
            }
        }
    }
}

fn client(sender: Sender<Messages>, mut stream: TcpStream)  {
    let mut buffer = Vec::new();
    buffer.resize(64, 0);

    stream.write(b"Seu nome: ").unwrap();

    let read_from_stream = stream.read(&mut buffer);
    let mut user = "".to_string();
    match read_from_stream {
        Ok(n) => {
            let buffer_slice = &buffer[0..n];
            let user_intro = b"Changing username to ".to_vec(); 
            let val = [user_intro, buffer_slice.to_vec().clone()].concat();
            stream.write(&val).unwrap();
            match String::from_utf8(buffer_slice.to_vec()) {
                Ok(username) => {
                    user = username
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
                sender.send(Messages::New(buffer_slice.to_vec(), cloned_user)).unwrap();
            }
            Err(..) => {
                println!("Disconnected");
                stream.shutdown(Shutdown::Both).expect("shutdown connection");
                break
            }
        }
    }
}
