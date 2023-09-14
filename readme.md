# sms3rs

## Workspace modules

- Server (implemented)
- Common (shared code for client and server)
- Client (implementing)

## Run and debug this project

1. Run the `run-prepare.sh` script for initialize the paths and files required for the system. (For other platforms such as `bat` script, feel free to contribute).
2. Run `cargo run` for the target package.

## Coding guidelines

### Server

The server use the [`axum`](https://docs.rs/axum/latest/axum/) web framework which uses [`tokio`](https://tokio.rs/).

Append docs to code if possible.
