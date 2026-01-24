use std::io::{self, Read, Write};

use xcbuild_bridge::{execute, OsRunner, RunRequest, RunResponse};

fn main() -> Result<(), io::Error> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let req: RunRequest = match serde_json::from_str(&input) {
        Ok(req) => req,
        Err(err) => {
            let resp = RunResponse::error(format!("json_parse_failed: {}", err));
            let out = serde_json::to_string(&resp).unwrap();
            io::stdout().write_all(out.as_bytes())?;
            std::process::exit(1);
        }
    };

    let resp = execute(req, &OsRunner);
    let out = serde_json::to_string(&resp).unwrap();
    io::stdout().write_all(out.as_bytes())?;
    if resp.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}
