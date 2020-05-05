use futures::future::join_all;
use reqwest::Client;
use std::io::{self, BufRead};
use std::sync::mpsc::{self, Sender};
use termion::{input::{MouseTerminal, TermRead}, screen::AlternateScreen, event::Key};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout},
    style::Color,
    widgets::{
        canvas::{Canvas, Map, MapResolution},
        Block, Borders,
    },
    Terminal,
};

#[derive(Debug, serde::Deserialize)]
struct IpInfo {
    ip: String,
    latitude: f64,
    longitude: f64,
    org: Option<String>,
    subdivision: Option<String>,
    subdivision2: Option<String>,
    city: Option<String>,
    country: Option<String>,
}

fn draw_ui<T: tui::backend::Backend>(terminal: &mut Terminal<T>, ip_infos: &Vec<IpInfo>) {
    terminal
        .draw(|mut f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(f.size());
            let canvas = Canvas::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("traceroute-vis"),
                )
                .paint(|ctx| {
                    ctx.draw(&Map {
                        color: Color::White,
                        resolution: MapResolution::High,
                    });
                    for info in ip_infos {
                        ctx.print(info.longitude, info.latitude, "x", Color::Yellow);
                    }
                })
                .x_bounds([-180.0, 180.0])
                .y_bounds([-90.0, 90.0]);
            f.render_widget(canvas, chunks[0]);
        })
        .unwrap();
}

async fn process(line: String, sender: &Sender<IpInfo>, client: &Client) -> () {
    let mut s = line.trim().split_whitespace();
    let _id = s.next();
    let _host = s.next();
    let ip = s.next().unwrap();
    if ip == "*" {
        return ();
    }
    let ip = &ip[1..ip.len() - 1];

    let response = client
        .get(("https://www.iplocate.io/api/lookup/".to_string() + &ip).as_str())
        .send()
        .await;
    let response = match response {
        Ok(r) => r,
        _ => return (),
    };
    let response: IpInfo = match response.json().await {
        Ok(r) => r,
        _ => return (),
    };
    sender.send(response).unwrap();
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let client = reqwest::Client::new();
    let stdin = std::io::stdin();

    let stdout = io::stderr();
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();

    let (tx, rx) = mpsc::channel();

    let handle = std::thread::spawn(move || {
        let mut ip_infos = vec![];
        draw_ui(&mut terminal, &ip_infos);
        for ip in rx {
            ip_infos.push(ip);
            draw_ui(&mut terminal, &ip_infos);
        }
        let stdin = termion::get_tty().unwrap();
        for event in stdin.keys() {
            if event.unwrap() == Key::Char('q') {
               return;
            }
        }
    });

    let mut tasks = vec![];
    for line in stdin.lock().lines().skip(1) {
        let line = match line {
            Ok(a) => a,
            _ => continue,
        };
        tasks.push(process(line, &tx, &client));
    }

    let _ = join_all(tasks).await;
    drop(tx);
    handle.join().unwrap();
    Ok(())
}
