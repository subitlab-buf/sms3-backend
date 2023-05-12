# sms3rs

## Workspace modules

- Server (partial implemented)
- Common (shared code for client and server, unimplemented)
- Client (unimplemented)
- Iced GUI (unimplemented)

## Run and debug this project

1. Run the `run-prepare.sh` script for initialize the paths and files required for the system. (For other platforms such as `bat` script, feel free to contribute).
2. Run `cargo run` for the target package.

## Coding guidelines

### Server

The server use the [`tide`](https://docs.rs/tide/latest/tide/) web framework which uses [`async-std`](https://async.rs/) async runtime / async implementation of rust std.

Append docs to code if possible.
