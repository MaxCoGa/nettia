use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream, IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::thread;
use serde::Deserialize;
use std::fs::File;

#[derive(Deserialize, Debug)]
// Structure to share connection status
struct ConnectionStatus {
    connected: bool,
}



#[derive(Deserialize, Debug)]
struct NodeDetails {
    name: String,
    ip: String,
    port: u16,
    tags: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct NodeConfig {
    node: NodeDetails,
}

fn read_node_config() -> Result<NodeConfig, Box<dyn std::error::Error + Send + Sync>> {
    let file = File::open("node.yml")?;
    let config: NodeConfig = serde_yaml::from_reader(file)?;

    Ok(config)
}


// Function to handle a single agent connection
fn handle_connection(mut stream: TcpStream, connection_status: Arc<Mutex<ConnectionStatus>>) -> io::Result<()> {
    // Get the address of the connected agent
    let peer_addr = match stream.peer_addr() {
        Ok(peer_addr) => peer_addr,
        Err(e) => {
            eprintln!("Error getting peer address: {}", e);
            return Err(e);
        }
    };
    println!("Agent connected from: {}", peer_addr);

    //Create an hasmap
    let mut agents_info: HashMap<String, String> = HashMap::new();


    // Read agent name
    let mut reader_name = BufReader::new(stream.try_clone()?);
    let mut agent_name = String::new();
    if let Err(e) = reader_name.read_line(&mut agent_name) {
        eprintln!("Error reading agent name: {}", e);
        return Err(e);
    }
    let agent_name = agent_name.trim().to_string();

    // Store agent name and IP address
    agents_info.insert(peer_addr.to_string(), agent_name.clone());
    println!("{} ({}) connected", agent_name, peer_addr);
    // Send the "connected" response
    if let Err(e) = stream.write_all(b"connected\n") {
        eprintln!("Error sending 'connected' message: {}", e);
        return Err(e);
    }

    // Flush the stream to ensure the data is sent
    if let Err(e) = stream.flush() {
        eprintln!("Error flushing stream: {}", e);
        return Err(e);
    }

    // Create a BufReader to read input from the agent
    let mut reader = BufReader::new(stream.try_clone()?);

    // Command dictionary
    let mut commands: HashMap<&str, &str> = HashMap::new();
    commands.insert("help", "Display available commands");
    commands.insert("nettia exit", "Exit the agent");

    // Infinite loop to handle agent's input
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // Connection closed by the agent
                println!("Agent disconnected from: {}", peer_addr);
                let mut status = connection_status.lock().unwrap();
                status.connected = false;
                return Ok(());
            }
            Ok(_) => {
                // Remove trailing newline
                let command = line.trim();

                // Print the received command
                println!("Received command: {} from {} ({})", command, agent_name, peer_addr);

                // Handle commands
                match command {
                    "nettia exit" => {
                        // The agent will exit by himself
                    }
                    "help" => {
                        // Send available commands to the agent
                        for (cmd, desc) in &commands {
                             if let Err(e) = stream.write_all(format!("{{\"{}\",\"{}\"}}\n", cmd, desc).as_bytes()) {
                                eprintln!("Error sending 'help' message: {}", e);
                            }
                            // Flush the stream to ensure the data is sent
                            if let Err(e) = stream.flush() {
                                eprintln!("Error flushing stream: {}", e);
                                return Err(e);
                            }
                        }
                       if let Err(e) = stream.write_all(b"\n") {
                         eprintln!("Error sending end message: {}", e);
                      }
                      if let Err(e) = stream.flush() {
                           eprintln!("Error flushing stream: {}", e);
                            return Err(e);
                      }
                    }
                    _ => {
                        // Command not found
                        println!("Command not found {} from {} ({})", command, agent_name,peer_addr);
                        
                        if let Err(e) = stream.write_all(format!("Command not found {}\n",command).as_bytes()) {                           
                           eprintln!("Error sending 'Command not found' message: {}", e);
                        }
                        if let Err(e) = stream.write_all(b"\n") {
                           eprintln!("Error sending end message: {}", e);
                        }
                        // Flush the stream to ensure the data is sent
                        if let Err(e) = stream.flush() {
                            eprintln!("Error flushing stream: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading from agent: {}", e);
                let mut status = connection_status.lock().unwrap();
                status.connected = false;
                return Err(e);
            }
        }
    }
}

fn main() -> io::Result<()> {

    let node_config = match read_node_config() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error reading config file: {}", e);
            return Err(io::Error::new(io::ErrorKind::Other, e));
        }
    };

    let node_ip: IpAddr = node_config.node.ip.parse().expect("Invalid IP address format");
    let node_port = node_config.node.port;

    // Shared connection status
    let connection_status = Arc::new(Mutex::new(ConnectionStatus { connected: true }));

    // Attempt to bind to the address
    let listener = match TcpListener::bind(SocketAddr::new(node_ip, node_port)) {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("Could not bind to address: {} port: {} {}", node_ip, node_port, e);
            return Err(e);
        }
    };
    println!("Nettia node listening on port {} for {}", node_ip, node_port);

    // Listen for incoming connections
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Clone connection status for the thread
                let connection_status_clone = Arc::clone(&connection_status);
                // Handle each connection in a separate thread
                thread::spawn(move || {
                    if let Err(e) = handle_connection(stream, connection_status_clone) {
                        eprintln!("Error handling connection: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}