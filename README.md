# Proxima backend

this is the backend server for the Proxima AI assistant, a personal, multi-device AI assistant and (in the future) a computer-use agent

This is currently a very experimental project and is not meant to be used in any sort of production or user-facing setting, you have been warned

## Security

This is currently using HTTP and does not feature security best practices. As such, it is not recommended for use outside of personal experimentation on private networks.

making this project more secure by using HTTPS or encrypting data over HTTP, as well as implementing more secure storage and treatment of secrets (such as passwords) will be a required step before reaching 1.0 

## Running

To build and run this program :
- clone this repository locally : 
```bash
git clone https://github.com/Pedro-W21/proxima_backend
```
- build the server : 
```bash
cd proxima_backend/proxima_backend_server
cargo build --release
```
- start an instance of your preferred inference engine interface offering a keyless OpenAI-compatible API (e.g. koboldcpp, LM Studio)

Now, you can start the proxima server in one of 2 ways :

### using the CLI setup

- run the server's binary : `./target/release/proxima_backend_server` (this path is relative from the `proxima_backend_server` directory, but you can put and execute the binary anywhere)
- go through the short setup process
- you're good to go ! The server will start and wait for connections

### using CLI arguments

- run the server's binary with the following arguments :
    - the username for this proxima instance
    - the password for this proxima instance
        - yes, it is given and stored in plaintext at the moment, DO NOT USE THIS FOR MORE THAN TESTING
    - the path to where you want proxima to store its persistent files
    - the URL pointing to your inference engine interface's OpenAI-compatible API
    - the port the server will open on

- example :
`./target/release/proxima_backend_server testname testpassword /path/to/proxima/data http://localhost:5001/v1/ 8082`