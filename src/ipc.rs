use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

/// 常駐インスタンスとの通信に使うループバック専用の固定ポート
const PORT: u16 = 48693;
const MSG_OPEN: &str = "BROWS-OPEN ";
const MSG_EXIT: &str = "BROWS-EXIT";
const MSG_ACK: &str = "BROWS-OK";

pub enum Request {
    Open(String),
    Exit,
}

/// ポートを確保できたら常駐インスタンスになれる（シングルインスタンスの判定を兼ねる）
pub fn try_bind() -> Option<TcpListener> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, PORT)).ok()
}

/// 常駐インスタンスへ URL を転送する。ack が返れば true
pub fn send_open(url: &str) -> bool {
    send(&format!("{}{}", MSG_OPEN, url))
}

/// 常駐インスタンスを終了させる（自動更新の再起動前に呼ぶ）
pub fn send_exit() -> bool {
    send(MSG_EXIT)
}

fn send(line: &str) -> bool {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, PORT));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(300)) else {
        return false;
    };
    // 常駐側がウィンドウ生成中だと ack まで待たされるため余裕を持たせる
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    if stream.write_all(format!("{}\n", line).as_bytes()).is_err() {
        return false;
    }
    let mut resp = String::new();
    if BufReader::new(stream).read_line(&mut resp).is_err() {
        return false;
    }
    resp.trim() == MSG_ACK
}

/// 常駐側: 1 接続を読み取って Request に変換し、ack を返す
pub fn read_request(stream: TcpStream) -> Option<Request> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let line = line.trim();

    let req = if let Some(url) = line.strip_prefix(MSG_OPEN) {
        if !url.starts_with("http") {
            return None;
        }
        Request::Open(url.to_string())
    } else if line == MSG_EXIT {
        Request::Exit
    } else {
        return None;
    };

    let mut stream = reader.into_inner();
    let _ = stream.write_all(format!("{}\n", MSG_ACK).as_bytes());
    Some(req)
}
