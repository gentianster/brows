use std::collections::VecDeque;

#[derive(Default)]
pub struct UrlQueue {
    pending: VecDeque<String>,
}

impl UrlQueue {
    pub fn push(&mut self, url: String) {
        self.pending.push_back(url);
    }

    pub fn take_next(&mut self, current_url: &str) -> Option<String> {
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
        queue.push("https://example.invalid/second".into());
        queue.push("https://example.invalid/third".into());
        assert_eq!(queue.take_next("https://example.invalid/first"), None);
        assert_eq!(
            queue.take_next(""),
            Some("https://example.invalid/second".into())
        );
        assert_eq!(queue.take_next("https://example.invalid/second"), None);
        assert_eq!(
            queue.take_next(""),
            Some("https://example.invalid/third".into())
        );
        assert!(queue.is_empty());
    }
}
