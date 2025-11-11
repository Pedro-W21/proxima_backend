use std::{env, io, path::PathBuf};

pub fn ask_for_input(input_text: &str) -> String {
    // similaire à input() de python
    // prend un prompt en entrée et sort un String
    let mut input = String::new();
    println!("{}", input_text);
    match io::stdin().read_line(&mut input) {
        Ok(_) => {
            ()
        },
        Err(e) => println!("Input reading error : {}", e)
    }
    input
}

pub struct InitializationData {
    pub username:String,
    pub password_hash:String,
    pub proxima_path:PathBuf,
    pub backend_url:String,
    pub port:u16,
}

pub fn initialize() -> InitializationData {
    let mut init = InitializationData { username: String::new(), password_hash: String::new(), proxima_path: PathBuf::new(), backend_url: String::new(), port:8082 };

    let args:Vec<String> = env::args().collect();

    if args.len() == 6 {
        let username = &args[1];
        let username_test = !username.trim().is_empty() && username.chars().collect::<Vec<char>>().len() < 100;
        let password = &args[2];
        let password_test = !password.trim().is_empty() && password.chars().collect::<Vec<char>>().len() < 100;
        let path_string = &args[3];
        let path_test = !path_string.trim().is_empty() && PathBuf::from(path_string.trim()).is_dir();
        let backend_url = &args[4];
        let url_test = !backend_url.trim().is_empty() && backend_url.chars().collect::<Vec<char>>().len() < 300;
        let port_str = &args[5];
        if let Ok(port) = port_str.parse::<u16>() && port > 1024 && port < 65_535 {
            init.port = port;
            if username_test && password_test && path_test && url_test {
                init.username = username.trim().to_string();
                init.password_hash = password.trim().to_string();
                init.proxima_path = PathBuf::from(path_string.trim()).join(PathBuf::from("proxima_backend/"));
                init.backend_url = backend_url.trim().to_string();
                return init;
            }
        }
    }

    println!("Hello, welcome to Proxima ! This is currently highly experimental, do not use this on a public network or with private information.");
    println!("To get you started, we'll need a username, a password, a path for persistent data, and a URL for the OpenAI-compatible LLM API used.");

    loop {
        let username = ask_for_input("What is your username ? It can be any string of up to 100 utf-8 characters. (This will be what Proxima will call you by default)");
        if !username.trim().is_empty() && username.chars().collect::<Vec<char>>().len() < 100 {
            init.username = username.trim().to_string();
            break;
        }
        else {
            println!("Username cannot be empty, and cannot be more than 100 characters long.")
        }
    }
    loop {
        let password = ask_for_input("What is your password ? It can be any string of up to 100 utf-8 characters.");
        if !password.trim().is_empty() && password.chars().collect::<Vec<char>>().len() < 100 {
            init.password_hash = password.trim().to_string();
            break;
        }
        else {
            println!("Password cannot be empty, and cannot be more than 100 characters long.")
        }
    }
    loop {
        let path_string = ask_for_input("What is The absolute path to your proxima persistent data ? It will create a new sub-folder in the target folder.");
        if !path_string.trim().is_empty() {
            let path_buf = PathBuf::from(path_string.trim());
            if path_buf.is_dir() {
                init.proxima_path = path_buf.join(PathBuf::from("proxima_backend/"));
                break;
            }
            else {
                println!("Path does not point to an existing folder.")
            }
        }
        else {
            println!("Path cannot be empty.")
        }
    }
    loop {
        let backend_url = ask_for_input("What is the OpenAI-compatible API URL ? This API cannot require an API key as of now.");
        if !backend_url.trim().is_empty() && backend_url.chars().collect::<Vec<char>>().len() < 300 {
            init.backend_url = backend_url.trim().to_string();
            break;
        }
        else {
            println!("URL cannot be empty, and cannot be longer than 300 characters long")
        }
    }
    loop {
        let port_str = ask_for_input("What is the port you want to use ? 8082 is the default port if no port is provided. The port must be a currently free port in the 1025-65,534 range");
        if port_str.trim().is_empty() {
            break;
        }
        else {
            if let Ok(port) = port_str.parse::<u16>() && port > 1024 && port < 65_534 {
                init.port = port;
                break;
            }
            else {
                println!("port cannot be empty, and must be a number between 1024 and 65534")
            }
        }
    }

    init
}