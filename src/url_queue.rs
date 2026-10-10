use std::collections::VecDeque;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct PendingUrl {
    pub url: String,
    pub launch_error: Option<String>,
}

#[derive(Default)]
pub struct UrlQueue {
    pending: VecDeque<PendingUrl>,
}

impl UrlQueue {
    pub fn push(&mut self, request: PendingUrl) {
        self.pending.push_back(request);
    }

    pub fn take_next(&mut self, current_url: &str) -> Option<PendingUrl> {
        if current_url.is_empty() {
            self.pending.pop_front()
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_visible_url_survives_a_burst_and_following_urls_keep_arrival_order() {
        let mut queue = UrlQueue::default();
        queue.push(PendingUrl {
            url: "https://example.invalid/second".into(),
            launch_error: None,
        });
        queue.push(PendingUrl {
            url: "https://example.invalid/third".into(),
            launch_error: None,
        });
        assert_eq!(queue.take_next("https://example.invalid/first"), None);
        assert_eq!(
            queue.take_next(""),
            Some(PendingUrl {
                url: "https://example.invalid/second".into(),
                launch_error: None,
            })
        );
        assert_eq!(queue.take_next("https://example.invalid/second"), None);
        assert_eq!(
            queue.take_next(""),
            Some(PendingUrl {
                url: "https://example.invalid/third".into(),
                launch_error: None,
            })
        );
        assert!(queue.is_empty());
    }

    #[test]
    fn a_queued_launch_error_stays_with_its_url_until_that_url_is_shown() {
        let mut queue = UrlQueue::default();
        queue.push(PendingUrl {
            url: "https://example.invalid/failed".into(),
            launch_error: Some("fixture launcher failed".into()),
        });
        queue.push(PendingUrl {
            url: "https://example.invalid/following".into(),
            launch_error: None,
        });
        assert_eq!(queue.take_next("https://example.invalid/current"), None);
        let failed = queue.take_next("").unwrap();
        assert_eq!(failed.url, "https://example.invalid/failed");
        assert_eq!(
            failed.launch_error.as_deref(),
            Some("fixture launcher failed")
        );
        assert_eq!(queue.take_next(&failed.url), None);
        let following = queue.take_next("").unwrap();
        assert_eq!(following.url, "https://example.invalid/following");
        assert_eq!(following.launch_error, None);
        assert!(queue.is_empty());
    }
}
