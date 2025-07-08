# Proxima backend

this is the backend server for the Proxima AI assistant, a personal, multi-device AI assistant and (in the future) a computer-use agent

This is currently a very experimental project and is not meant to be used in any sort of production or user-facing setting, you have been warned

## Security

This is currently using HTTP and does not feature security best practices. As such, it is not recommended for use outside of personal experimentation on private networks.

making this project more secure by using HTTPS or encrypting data over HTTP, as well as implementing more secure storage and treatment of secrets (such as passwords) will be a required step before reaching 1.0 

## Running

To build and run this program :
- clone this repository locally
- start an instance of your preferred inference engine offering a keyless OpenAI-compatible API
- make sure port 8082 is not currently in use
- run `cargo run --release` in the "proxima_backend_server" folder
- go through the short setup process
- you're good to go ! The server will open on the port 8082