#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngestLimits {
    pub max_raw_mime_bytes: usize,
    pub max_raw_mail_object_bytes: usize,
    pub max_recent_raw_mail_bytes: usize,
    pub recent_raw_mail_window_seconds: i64,
    pub max_mime_depth: usize,
    pub max_attachment_count: usize,
}

impl IngestLimits {
    pub const DEFAULT_MAX_RAW_MIME_BYTES: usize = 25 * 1024 * 1024;
    pub const DEFAULT_MAX_RAW_MAIL_OBJECT_BYTES: usize = 10 * 1024 * 1024;
    pub const DEFAULT_MAX_RECENT_RAW_MAIL_BYTES: usize = 50 * 1024 * 1024;
    pub const DEFAULT_RECENT_RAW_MAIL_WINDOW_SECONDS: i64 = 60 * 60;

    pub const fn new(
        max_raw_mime_bytes: usize,
        max_mime_depth: usize,
        max_attachment_count: usize,
    ) -> Self {
        Self {
            max_raw_mime_bytes,
            max_raw_mail_object_bytes: Self::DEFAULT_MAX_RAW_MAIL_OBJECT_BYTES,
            max_recent_raw_mail_bytes: Self::DEFAULT_MAX_RECENT_RAW_MAIL_BYTES,
            recent_raw_mail_window_seconds: Self::DEFAULT_RECENT_RAW_MAIL_WINDOW_SECONDS,
            max_mime_depth,
            max_attachment_count,
        }
    }

    pub const fn with_raw_mail_controls(
        mut self,
        max_raw_mail_object_bytes: usize,
        max_recent_raw_mail_bytes: usize,
        recent_raw_mail_window_seconds: i64,
    ) -> Self {
        self.max_raw_mail_object_bytes = max_raw_mail_object_bytes;
        self.max_recent_raw_mail_bytes = max_recent_raw_mail_bytes;
        self.recent_raw_mail_window_seconds = recent_raw_mail_window_seconds;
        self
    }
}

impl Default for IngestLimits {
    fn default() -> Self {
        Self {
            max_raw_mime_bytes: Self::DEFAULT_MAX_RAW_MIME_BYTES,
            max_raw_mail_object_bytes: Self::DEFAULT_MAX_RAW_MAIL_OBJECT_BYTES,
            max_recent_raw_mail_bytes: Self::DEFAULT_MAX_RECENT_RAW_MAIL_BYTES,
            recent_raw_mail_window_seconds: Self::DEFAULT_RECENT_RAW_MAIL_WINDOW_SECONDS,
            max_mime_depth: 20,
            max_attachment_count: 25,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::IngestLimits;

    #[test]
    fn inbound_limits_default_limits_match_confirmed_m8_values() {
        let limits = IngestLimits::default();

        assert_eq!(limits.max_raw_mime_bytes, 25 * 1024 * 1024);
        assert_eq!(limits.max_raw_mail_object_bytes, 10 * 1024 * 1024);
        assert_eq!(limits.max_recent_raw_mail_bytes, 50 * 1024 * 1024);
        assert_eq!(limits.recent_raw_mail_window_seconds, 60 * 60);
        assert_eq!(limits.max_mime_depth, 20);
        assert_eq!(limits.max_attachment_count, 25);
    }

    #[test]
    fn inbound_limits_accept_custom_test_limits() {
        let limits = IngestLimits::new(1024, 2, 1);

        assert_eq!(limits.max_raw_mime_bytes, 1024);
        assert_eq!(limits.max_raw_mail_object_bytes, 10 * 1024 * 1024);
        assert_eq!(limits.max_recent_raw_mail_bytes, 50 * 1024 * 1024);
        assert_eq!(limits.recent_raw_mail_window_seconds, 60 * 60);
        assert_eq!(limits.max_mime_depth, 2);
        assert_eq!(limits.max_attachment_count, 1);
    }

    #[test]
    fn inbound_limits_accept_custom_raw_mail_controls() {
        let limits = IngestLimits::new(1024, 2, 1).with_raw_mail_controls(512, 2048, 300);

        assert_eq!(limits.max_raw_mail_object_bytes, 512);
        assert_eq!(limits.max_recent_raw_mail_bytes, 2048);
        assert_eq!(limits.recent_raw_mail_window_seconds, 300);
    }
}
