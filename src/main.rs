use std::net::{Shutdown, TcpListener, TcpStream};
use std::io::Read;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

enum Messages {
    New(Vec<u8>),
    Disconnect,
}

fn main() {
    let address = "127.0.0.1:7777";

    let listener = TcpListener::bind(address).unwrap_or_else(|error| {
        println!("{:?}", error);
        panic!("Could no bind to {}", &address)
    });
    
    let (sender, receiver): (Sender<Messages> , Receiver<Messages>) = channel();

    let mut users: Vec<String> = Vec::new();

    thread::spawn(move || server(receiver));
    
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let sender = sender.clone();
                let author_address = stream.peer_addr().unwrap();
                let user = author_address.to_string();
                users.push(user);
                println!("Connection established");
                thread::spawn(|| client(sender, stream));
            }
            Err(_e) => {
                panic!("Could not read stream")
            }
        };
        println!("{:?}", users);
    };
}


fn server(receiver: Receiver<Messages>) {
    loop {
        let messages = receiver.recv().unwrap();
        match messages {
            Messages::New(values) => {
                let string_value = String::from_utf8(values).unwrap_or_else(|error| {
                    println!("{:?}", error);
                    println!("Erro não foi possivel ler");
                    "".to_string()
                });
                println!("Response: {}", string_value);
            }
            Messages::Disconnect => {
                todo!()
            }
        }
    }
}

fn client(sender: Sender<Messages>, mut stream: TcpStream)  {
    println!("Está conectado");
    let mut buffer = Vec::new();
    buffer.resize(64, 0);
    loop {
        let read_from_stream = stream.read(&mut buffer);
        println!("{:?}", read_from_stream);
        match read_from_stream {
            Ok(n) => {
                if n == 0 {
                    sender.send(Messages::Disconnect);
                    break
                };
                sender.send(Messages::New(buffer[0..n].to_vec()));
            }
            Err(..) => {
                println!("Disconnected");
                stream.shutdown(Shutdown::Both).expect("shutdown connection");
                break
            }
        }
    }
}
