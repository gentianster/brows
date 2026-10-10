#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{send_exit, send_open, send_open_wait, try_bind, PipeServer, Request};
#[cfg(windows)]
pub use windows::{send_exit, PipeServer, Request};

#[cfg(windows)]
pub fn send_open(url: &str) -> anyhow::Result<bool> {
    Ok(windows::send_open(url))
}

#[cfg(windows)]
pub fn try_bind() -> anyhow::Result<Option<PipeServer>> {
    Ok(windows::try_bind())
}

#[cfg(windows)]
pub fn send_open_wait(url: &str) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if windows::send_open(url) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

pub fn is_web_url(value: &str) -> bool {
    !value.chars().any(char::is_control)
        && url::Url::parse(value)
            .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}

#[cfg(test)]
mod tests {
    #[test]
    fn accepts_only_complete_http_and_https_urls() {
        for url in [
            "https://example.com/日本語?a=1&b=2",
            "HTTP://example.com",
            "http://localhost:3000",
        ] {
            assert!(super::is_web_url(url), "{url}");
        }
        for url in [
            "httpwhatever",
            "http://",
            "file:///tmp/foo",
            "https://example.com\nBROWS-EXIT",
            "--register",
        ] {
            assert!(!super::is_web_url(url), "{url}");
        }
    }
}
