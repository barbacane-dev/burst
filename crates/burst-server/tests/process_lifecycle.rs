//! The server as a process: does it start, serve, and stop cleanly?
//!
//! Every other test drives the router in-process, so nothing here ever started
//! a real binary, sent it a signal and looked at its exit code. That is how a
//! panic on graceful shutdown reaches a release: a supervisor reads a non-zero
//! exit as a crash, so restart backoff, crash-loop counters and alerts all fire
//! on an ordinary stop.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Wait until `port` answers, or give up.
fn wait_for_port(port: u16, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// The binary under test, built by the same `cargo test` invocation.
fn burst_binary() -> std::path::PathBuf {
    let mut dir = std::env::current_exe().expect("test binary path");
    dir.pop(); // deps/
    dir.pop(); // debug/ or release/
    dir.join("burst")
}

/// A config that binds ports nothing else in the suite uses.
fn write_config(
    dir: &std::path::Path,
    port: u16,
    admin_port: u16,
    db_url: &str,
) -> std::path::PathBuf {
    let path = dir.join("burst.toml");
    std::fs::write(
        &path,
        format!(
            r#"[server]
listen = "127.0.0.1:{port}"
admin_listen = "127.0.0.1:{admin_port}"

[database]
url = "{db_url}"
max_connections = 5

[auth]
mode = "trusted-headers"
trusted_proxies = ["127.0.0.1/32"]
"#
        ),
    )
    .expect("write config");
    path
}

/// SIGTERM on a healthy process must drain and exit 0. A non-zero exit is how
/// a supervisor decides the process crashed.
#[sqlx::test(migrations = "../../migrations")]
async fn sigterm_on_a_serving_process_exits_zero(pool: sqlx::PgPool) {
    let _ = &pool;
    let db_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("skipping: DATABASE_URL is not set");
            return;
        }
    };
    let binary = burst_binary();
    if !binary.exists() {
        eprintln!("skipping: {} is not built", binary.display());
        return;
    }

    let dir = tempfile::tempdir().expect("temp dir");
    let config = write_config(dir.path(), 34117, 34118, &db_url);

    let mut child = Command::new(&binary)
        .arg(&config)
        .env("RUST_LOG", "warn")
        .current_dir(dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the server");

    assert!(
        wait_for_port(34117, Duration::from_secs(30)),
        "the server never accepted a connection"
    );

    // SIGTERM, the signal every supervisor sends first.
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }

    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        match child.try_wait().expect("wait") {
            Some(status) => break status,
            None if Instant::now() > deadline => {
                let _ = child.kill();
                panic!("the server did not exit within 30s of SIGTERM");
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    };

    let mut stderr = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut stderr);
    }

    assert!(
        !stderr.contains("panicked"),
        "the server panicked while shutting down:\n{stderr}"
    );
    assert_eq!(
        status.code(),
        Some(0),
        "SIGTERM must exit 0, got {status:?}\nstderr:\n{stderr}"
    );
}

/// A port already taken is a configuration error, and must be reported as one
/// rather than as a crash, so the two stay distinguishable by exit code.
#[sqlx::test(migrations = "../../migrations")]
async fn a_taken_port_fails_without_panicking(pool: sqlx::PgPool) {
    let _ = &pool;
    let db_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => return,
    };
    let binary = burst_binary();
    if !binary.exists() {
        return;
    }

    // Hold the port so the server cannot have it.
    let listener = std::net::TcpListener::bind("127.0.0.1:34119").expect("hold the port");

    let dir = tempfile::tempdir().expect("temp dir");
    let config = write_config(dir.path(), 34119, 34120, &db_url);

    let output = Command::new(&binary)
        .arg(&config)
        .env("RUST_LOG", "warn")
        .current_dir(dir.path())
        .output()
        .expect("run the server");

    drop(listener);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Address already in use") || stderr.contains("AddrInUse"),
        "the real cause must be reported:\n{stderr}"
    );
    assert!(
        !stderr.contains("panicked"),
        "a taken port is a configuration error, not a panic:\n{stderr}"
    );
}
