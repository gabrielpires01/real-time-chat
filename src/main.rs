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
    Command(Commands),
}

enum Commands {
    ChangeGroup(String, String),
    PrivateMessage(Vec<u8>, String, String),
    ListCommands(String),
    ListGroups(String),
    CreateGroup(String, String),
}

const BLUE: &str = "\x1b[34m";
const GREEN: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";

fn main() {
    let address = "127.0.0.1:7777";

    let listener = TcpListener::bind(address).unwrap_or_else(|error| {
        println!("{:?}", error);
        panic!("Could no bind to {}", &address)
    });
    
    let (sender, receiver): (Sender<Messages> , Receiver<Messages>) = channel();

    let users: Arc<Mutex<HashMap<String, TcpStream>>> = Arc::new(Mutex::new(HashMap::new()));
    let groups: Arc<Mutex<HashMap<String, Vec<String>>>> = Arc::new(Mutex::new(HashMap::new()));
    let user_group: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    groups.lock().unwrap().insert("main".to_string(), Vec::new());

    {
        let users_clone = Arc::clone(&users);
        let groups_clone = Arc::clone(&groups);
        let user_group_clone = Arc::clone(&user_group);
        thread::spawn(move || server(receiver, users_clone, groups_clone, user_group_clone));
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

fn server(
    receiver: Receiver<Messages>, 
    users: Arc<Mutex<HashMap<String, TcpStream>>>, 
    groups: Arc<Mutex<HashMap<String, Vec<String>>>>, 
    user_group: Arc<Mutex<HashMap<String, String>>>
) {
    loop {
        let messages = receiver.recv().unwrap();
        match messages {
            Messages::New(values, user) => {
                let user_groups = user_group.lock().unwrap();
                let u_group = user_groups.get(&user).unwrap();
                let groups_map = groups.lock().unwrap();
                let users_from_group = groups_map.get(u_group).unwrap();
                let users_map = users.lock().unwrap();

                let mut group_user = {
                    users_from_group.iter().filter(|user_from_group| user != **user_from_group).map(|user_from_group| {
                        let user_stream = users_map.get(user_from_group).unwrap();
                        user_stream 
                    }).collect::<Vec<&TcpStream>>()
                };
                for stream in group_user.iter_mut() {
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
                let cloned_user = user.clone();
                users_map.insert(user, stream);
                groups.lock().unwrap().get_mut("main").unwrap().push(cloned_user.clone());
                user_group.lock().unwrap().insert(cloned_user, "main".to_string());
            }
            Messages::Command(commands) => {
                let users_map = users.lock().unwrap();
                match commands {
                    Commands::ChangeGroup(to_group, user) => {
                        let cloned_user = user.clone();
                        let mut to_stream = users_map.get(&user).unwrap();
                        let from_group = user_group.lock().unwrap().get(&user).unwrap().clone();

                        groups.lock().unwrap().get_mut(&from_group).unwrap().retain(|x| *x != user);
                        groups.lock().unwrap().get_mut(&to_group).unwrap().push(user);
                        user_group.lock().unwrap().insert(cloned_user, to_group.clone());

                        to_stream.write(format!("Group changed to {}\n", to_group).as_bytes()).unwrap();
                    }
                    Commands::PrivateMessage(values, from_user, to_user) => {
                        let mut to_stream = users_map.get(&to_user).unwrap();
                        let private_user = format!("{}Private message from {}: {}", BLUE, from_user.trim(), RESET).as_bytes().to_vec(); 
                        let private_val = [private_user, values.clone()].concat();
                        to_stream.write(&private_val).unwrap();
                    }
                    Commands::ListCommands(user) => {
                        let mut to_stream = users_map.get(&user).unwrap();
                        to_stream.write(b"Commands: \n").unwrap();
                        to_stream.write(b"/join [group] - join group\n").unwrap();
                        to_stream.write(b"/create [group] - create group\n").unwrap();
                        to_stream.write(b"/p [user] [message] - private message\n").unwrap();
                        to_stream.write(b"/g - list groups\n").unwrap();
                        to_stream.write(b"/h - list commands\n").unwrap();
                    }
                    Commands::ListGroups(user) => {
                        let mut to_stream = users_map.get(&user).unwrap();
                        let groups_map = groups.lock().unwrap();
                        let mut groups_list = groups_map.keys().collect::<Vec<&String>>();
                        groups_list.sort();
                        to_stream.write(b"Groups: \n").unwrap();
                        for group in groups_list {
                            to_stream.write(format!("{}{}{}\n", GREEN, group, RESET).as_bytes()).unwrap();
                        }
                    }
                    Commands::CreateGroup(group, user) => {
                        let cloned_user = user.clone();
                        let clone_group = group.clone();
                        let mut to_stream = users_map.get(&user).unwrap();
                        groups.lock().unwrap().insert(group, Vec::new());
                        to_stream.write(format!("{}Group Created: {}{}\n", GREEN, clone_group, RESET).as_bytes()).unwrap();
                        println!("Created group {} by {}", clone_group, cloned_user);
                    }
                }
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
                        if message.starts_with("/") {
                            let split_message = message.split(" ").collect::<Vec<&str>>();
                            match split_message[0].trim() {
                                "/join" => {
                                    if split_message.len() != 2 {
                                        stream.write(b"Invalid message. eg: /join group\n").unwrap();
                                        return
                                    }
                                    let rest_message = split_message[1].trim().to_string();
                                    sender.send(Messages::Command(Commands::ChangeGroup(rest_message, cloned_user.clone()))).unwrap();
                                }
                                "/create" => {
                                    if split_message.len() != 2 {
                                        stream.write(b"Invalid message. eg: /create group\n").unwrap();
                                        return
                                    }
                                    let rest_message = split_message[1].trim().to_string();
                                    sender.send(Messages::Command(Commands::CreateGroup(rest_message, cloned_user.clone()))).unwrap();
                                }
                                "/p" => {
                                    if split_message.len() < 3 {
                                        stream.write(b"Invalid message. eg: /p user message\n").unwrap();
                                        return
                                    }
                                    let to_user = split_message[1].trim().to_string();
                                    sender.send(Messages::Command(Commands::PrivateMessage(split_message[2..].join(" ").as_bytes().to_vec(), cloned_user.clone(), to_user))).unwrap();
                                }
                                "/h" => {
                                    sender.send(Messages::Command(Commands::ListCommands(cloned_user.clone()))).unwrap();
                                }
                                "/g" => {
                                    sender.send(Messages::Command(Commands::ListGroups(cloned_user.clone()))).unwrap();
                                }
                                _ => {
                                    stream.write(b"Invalid command\n").unwrap();
                                }
                                
                            }
                        } else {
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
