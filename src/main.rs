use std::io;

use xcbuild_bridge::run_stdio_server;

fn main() -> Result<(), io::Error> {
    run_stdio_server()
}
