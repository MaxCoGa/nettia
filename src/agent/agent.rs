use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;
use serde::Deserialize;
use serde_yaml;
use std::net::SocketAddr;


#[derive(Deserialize, Debug)]
struct AgentConfig {
    agent: AgentMainDetails,
    node: Vec<NodeDetails>
}

#[derive(Deserialize, Debug)]
struct AgentMainDetails {

    name: String,
}


#[derive(Deserialize, Debug)]
struct NodeDetails {
    name: String,
    ip: String,
    port: u16,
    tags: Vec<String>
}


fn send_agent_name(stream: &mut TcpStream, agent_name: &str) -> io::Result<()> {
    if let Err(e) = stream.write_all(format!("{}\n", agent_name).as_bytes()) {
        eprintln!("Error writing agent name to node: {}", e);
        return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
    }
    stream.flush()?;
    Ok(())
}

fn main() -> io::Result<()> {
    // Read and parse the agent.yml file
    let file = std::fs::File::open("agent.yml")?;
    let config: AgentConfig = match serde_yaml::from_reader(file) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error parsing agent.yml: {}", e);
            return Err(io::Error::new(io::ErrorKind::Other, "Failed to parse agent.yml"));
        }
    };
    
    let agent_name = &config.agent.name;
    // Assuming you want to use the first node's details for connection
    let first_node = config.node.get(0).ok_or_else(|| {
        io::Error::new(io::ErrorKind::Other, "No nodes defined in agent.yml")
    })?;
    
    let node_ip = &first_node.ip;
    let node_port = first_node.port;
    let node_address = SocketAddr::from((node_ip.parse::<std::net::IpAddr>().unwrap(), node_port));

    
    // Attempt to connect to the node with a timeout
    let mut stream = match TcpStream::connect_timeout(
        &node_address,
        Duration::from_secs(5), )

    {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!("Failed to connect to node {}: {}", node_address, e);
            return Err(e);
        }
    };

    // Send the agent's name to the node
    send_agent_name(&mut stream, agent_name)?;

    println!("Connected to node: {}", &node_address);

    // Create a BufReader for reading from the node
    let mut reader = BufReader::new(stream.try_clone()?);

    // Read the initial "connected" response
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(_) => print!("Node response: {}", line),
        Err(e) => {
            eprintln!("Failed to read from node: {}", e);
            return Err(e);
        }
    };


    // Input loop
    loop {
        // Read input from the user
        let mut input = String::new();
        if let Err(e) = io::stdin().read_line(&mut input) {
            eprintln!("Failed to read line: {}", e);
            continue;
        }

        print!("{}: ", &agent_name);
         io::stdout().flush().unwrap();


        // Remove trailing newline
        let command = input.trim();

        if command == "nettia exit" {
            println!("Exiting...");
            break;
        }

        // Send the command to the node
        if let Err(e) = stream.write_all(input.as_bytes()) {
            eprintln!("Error writing to node: {}", e);
            break;
        }
        // Flush the stream to ensure the data is sent
        if let Err(e) = stream.flush() {
            eprintln!("Error flushing stream: {}", e);
            break;
        }
        // Display node messages
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    // Connection closed by the node
                    break;
                }
                Ok(_) => {
                    if line == "\n"{
                       break;
                    }
                    print!("Node: {}", line);
                }
                Err(e) => {
                    eprintln!("Failed to read from node: {}", e);
                    break;
                }
            }
        }
    }
    Ok(())
}