use chrono::Local;
use clap::{App, Arg};
use env_logger::Builder;
use log::{debug, LevelFilter};
use reqwest;
use rusqlite::{params, Connection, Result};
use std::fs;
use std::io::{ErrorKind, Write};
use std::thread;
use std::time::{Duration, Instant};
use tokio;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = App::new("My Rust Program")
        .version("1.0")
        .author("Your Name")
        .about("Does awesome things")
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .help("Activates debug mode")
                .takes_value(false),
        )
        .arg(Arg::new("every").short('e').long("every").takes_value(true))
        .get_matches();
    let every_sec = if matches.is_present("every") {
        matches.value_of("every").unwrap()
    } else {
        "60"
    };
    let every_sec_u64 = every_sec.parse::<u64>().unwrap();
    let log_level = if matches.is_present("debug") {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    Builder::new()
        .filter(None, log_level)
        .format(|buf, record| writeln!(buf, "{}: {}", record.level(), record.args()))
        .init();
    loop {
        let now = Local::now();
        let urls = read_urls_from_file("targets.txt");
        debug!("Current time: {}", now.format("%Y-%m-%d %H:%M:%S"));
        let conn = Connection::open("my_database.db")?;
        let table_exists = conn
            .query_row(
                "SELECT count(*) FROM request_data WHERE type='table' AND name='request_data'",
                [],
                |row| row.get::<_, i32>(0),
            )
            .unwrap_or(0)
            > 0;
        if !table_exists {
            conn.execute(
                "CREATE TABLE IF NOT EXISTS request_data (
                        id INTEGER PRIMARY KEY,
                        timestamp TIMESTAMP NOT NULL,
                        target TEXT NOT NULL,
                        connection_time FLOAT,
                        status_code INTEGER NOT NULL,
                        response_text TEXT,
                        header_text TEXT
                        )",
                [],
            )?;
        }
        for url in urls {
            fetch_and_store(url.as_str(), &conn).await?;
        }

        debug!("Sleep {} seconds", every_sec);
        thread::sleep(Duration::from_secs(every_sec_u64));
    }
}

async fn fetch_and_store(url: &str, conn: &Connection) -> Result<()> {
    // Make a GET request
    debug!("{}", format!("Going to curl {}", url));
    let start = Instant::now();
    let res = reqwest::get(url).await.unwrap();

    // Get the status code and the body
    let status_code = res.status().as_u16();
    let headers = res.headers().clone();
    let body = res.text().await.unwrap();

    // Convert the headers into a string
    let headers_str = headers
        .iter()
        .map(|(key, value)| format!("{}: {:?}", key, value))
        .collect::<Vec<String>>()
        .join("\n");

    let duration = start.elapsed().as_millis() as f64;
    // Insert data into the table
    conn.execute(
        "INSERT INTO request_data (status_code, timestamp, target, connection_time, response_text, header_text) VALUES (?1, datetime('now'), ?2, ?3, ?4, ?5)",
        params![status_code, url, duration, body, headers_str],
    )?;

    Ok(())
}

fn read_urls_from_file(file_path: &str) -> Vec<String> {
    match fs::read_to_string(file_path) {
        Ok(content) => content.lines().map(|s| s.to_string()).collect(),
        Err(e) => {
            match e.kind() {
                ErrorKind::NotFound => {
                    eprintln!("File not found: {}", file_path);
                }
                _ => {
                    eprintln!("Error reading file: {}", e);
                }
            }
            Vec::new() // Return an empty vector if there's an error
        }
    }
}
