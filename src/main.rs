use openssl::ssl::{SslConnector, SslMethod, SslStream};
use std::fs;
use std::io;
use std::io::prelude::*;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};

enum Port {
    Custom(u16),
    Http,
    Https,
}

enum Stream {
    HttpsStream(SslStream<TcpStream>),
    HttpStream(TcpStream),
}

impl From<&Port> for String {
    fn from(port: &Port) -> String {
        match port {
            Port::Http => String::from("80"),
            Port::Https => String::from("443"),
            Port::Custom(p) => String::from(format!("{}", p)),
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Stream::HttpStream(ref mut http) => http.read(buf),
            Stream::HttpsStream(ref mut https) => https.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Stream::HttpStream(ref mut http) => http.write(buf),
            Stream::HttpsStream(ref mut https) => https.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Stream::HttpStream(ref mut http) => http.flush(),
            Stream::HttpsStream(ref mut https) => https.flush(),
        }
    }
}

struct Connection {
    stream: Stream,
    url: Url,
}

struct Url {
    path: String,
    host: String,
    port: Port,
}

impl Url {
    fn new(mut url: String) -> Url {
        let lower_case_url = url.trim().to_lowercase();

        let port = if lower_case_url.contains("https") {
            let i = match lower_case_url.find("https") {
                Some(n) => n,
                None => {
                    println!("Url must be valid");
                    std::process::exit(0);
                }
            };

            url = url[i + "https://".len()..].to_string();
            Port::Https
        } else if lower_case_url.contains("http") {
            let i = match lower_case_url.find("http") {
                Some(n) => n,
                None => {
                    println!("Url must be valid");
                    std::process::exit(0);
                }
            };

            url = url[i + "http://".len()..].to_string();
            Port::Http
        } else if lower_case_url.contains(":") {
            let split_str: Vec<&str> = lower_case_url.split(':').collect();
            url = match split_str.get(0) {
                Some(s) => s.to_string(),
                None => String::new(),
            };

            let port = match split_str
                .get(1)
                .and_then(|port_and_path| port_and_path.split('/').next())
                .and_then(|port_str| port_str.parse::<u16>().ok())
            {
                Some(p) => p,
                None => {
                    println!("Url must be valid.");
                    std::process::exit(0);
                }
            };
            Port::Custom(port)
        } else {
            Port::Http
        };

        let split_str: Vec<&str> = url.trim().split('/').collect();

        let host = split_str.get(0).unwrap().to_string();
        let path = split_str.get(1).unwrap_or(&"").to_string();

        Url { host, path, port }
    }

    fn get_addr(&self) -> SocketAddr {
        let connection_str = format!("{}:{}", self.host, String::from(&self.port));
        let mut addr = match connection_str.to_socket_addrs() {
            Ok(c) => c,
            Err(_) => {
                println!("Url must be valid.");
                std::process::exit(0);
            }
        };

        let addr = match addr.next() {
            Some(c) => c,
            None => {
                println!("Url must be valid.");
                std::process::exit(0);
            }
        };

        addr
    }
}

impl Connection {
    fn http(url: Url) -> Connection {
        let addr = url.get_addr();

        let stream = match TcpStream::connect(addr) {
            Ok(s) => s,
            Err(_) => {
                println!("HTTP: Couldn't connect.");
                std::process::exit(0);
            }
        };

        Connection {
            stream: Stream::HttpStream(stream),
            url,
        }
    }

    fn https(url: Url) -> Connection {
        let connector = match SslConnector::builder(SslMethod::tls()) {
            Ok(c) => c,
            Err(_) => {
                println!("SSL: connector error.");
                std::process::exit(0);
            }
        }
        .build();

        let stream = match TcpStream::connect(url.get_addr()) {
            Ok(s) => s,
            Err(_) => {
                println!("SSL: Couldn't connect.");
                std::process::exit(0);
            }
        };

        let ssl_stream = match connector.connect(url.host.as_str(), stream) {
            Ok(s) => s,
            Err(_) => {
                println!("SSL: couldn't establish an SSL connection.");
                std::process::exit(0);
            }
        };

        Connection {
            stream: Stream::HttpsStream(ssl_stream),
            url,
        }
    }
}

fn connect() -> Connection {
    let mut input = String::new();
    println!("Enter URL or paste text:");
    match io::stdin().read_line(&mut input) {
        Ok(_) => (),
        Err(_) => {
            println!("Input must be valid.");
        }
    };

    let url = Url::new(input);

    let connection = match url.port {
        Port::Http => Connection::http(url),
        Port::Https => Connection::https(url),
        Port::Custom(_) => Connection::http(url),
    };

    connection
}

fn req_without_body(method: &str) {
    let mut connection = connect();
    let request = format!(
        "{} /{} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        method, connection.url.path, connection.url.host
    );

    match connection.stream.write_all(request.as_bytes()) {
        Ok(_n) => _n,
        Err(_) => {
            println!("Write error");
            std::process::exit(0);
        }
    };

    let mut buf = [0; 1024];

    while let Ok(bytes_read) = connection.stream.read(&mut buf) {
        if bytes_read == 0 {
            break;
        }
        println!("{}", String::from_utf8_lossy(&buf[..bytes_read]));
    }

    std::process::exit(0);
}

fn req_with_body(method: &str) {
    let mut connection = connect();
    let file = match fs::read_to_string("src/body.json") {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Failed to read body.json: {}", e);
            std::process::exit(1);
        }
    };
    let request = format!(
        "{} /{} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        method, connection.url.path, connection.url.host, file.len(), file
    );

    if let Err(_) = connection.stream.write_all(request.as_bytes()) {
        println!("Write error");
        std::process::exit(1);
    };

    let mut buf = [0; 1024];

    while let Ok(bytes_read) = connection.stream.read(&mut buf) {
        if bytes_read == 0 {
            break;
        }
        println!("{}", String::from_utf8_lossy(&buf[..bytes_read]));
    }

    std::process::exit(0);
}

fn get() {
    req_without_body("GET");
}

fn delete() {
    req_without_body("DELETE");
}

fn post() {
    req_with_body("POST");
}
fn put() {
    req_with_body("PUT");
}

fn menu_screen() {
    print!("\n-------------------- HTTP Client Menu --------------------\n\n");
    print!("\t\t1. Send GET request\n");
    print!("\t\t2. Send POST request\n");
    print!("\t\t3. Send PUT request\n");
    print!("\t\t4. Send DELETE request\n");
    print!("\t\t5. Exit\n");
}
fn clear() {
    let _ = std::process::Command::new("clear").status();
}

fn handle_input(input: usize) {
    if input == 1 {
        clear();
        get();
    } else if input == 2 {
        clear();
        post();
    } else if input == 3 {
        clear();
        put();
    } else if input == 4 {
        clear();
        delete();
    } else if input == 5 {
        std::process::exit(0);
    }
}

fn handle_user() {
    loop {
        menu_screen();
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => (),
            Err(_) => {
                println!("Input must be valid.");
                continue;
            }
        };

        let input: usize = match input.trim().parse() {
            Ok(n) => {
                if n > 5 || n < 1 {
                    println!("Input must be between 1 and 5.\n");
                    continue;
                }
                n
            }
            Err(_) => {
                println!("Input must be a number.");
                continue;
            }
        };

        handle_input(input);
    }
}

fn main() {
    handle_user();
}
