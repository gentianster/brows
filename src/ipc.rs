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
    windows::send_open(url)
}

#[cfg(windows)]
pub fn try_bind() -> anyhow::Result<Option<PipeServer>> {
    Ok(windows::try_bind())
}

#[cfg(windows)]
pub fn send_open_wait(url: &str) -> anyhow::Result<bool> {
    retry_not_ready(
        || windows::send_open(url),
        std::time::Duration::from_secs(5),
    )
}

fn retry_not_ready(
    mut send: impl FnMut() -> anyhow::Result<bool>,
    timeout: std::time::Duration,
) -> anyhow::Result<bool> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        // An error after connection may follow an accepted URL, so it must not be retried.
        if send()? {
            return Ok(true);
        }
        if std::time::Instant::now() >= deadline {
            return Ok(false);
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
    fn uncertain_delivery_is_reported_without_resending_the_url() {
        let mut attempts = 0;
        let result = super::retry_not_ready(
            || {
                attempts += 1;
                anyhow::bail!("fixture request accepted without ACK")
            },
            std::time::Duration::from_secs(1),
        );
        assert!(result.is_err());
        assert_eq!(attempts, 1);
    }

    #[test]
    fn an_unconnected_resident_can_be_retried_until_it_acknowledges() {
        let mut attempts = 0;
        let result = super::retry_not_ready(
            || {
                attempts += 1;
                Ok(attempts == 2)
            },
            std::time::Duration::from_secs(1),
        );
        assert!(result.unwrap());
        assert_eq!(attempts, 2);
    }

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
