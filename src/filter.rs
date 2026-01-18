use crate::error::Result;
use crate::models::Notification;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct Filter {
    pattern: Option<Regex>,
}

impl Filter {
    pub fn new(pattern: Option<&str>) -> Result<Self> {
        Ok(Self {
            pattern: match pattern {
                Some(p) => Some(Regex::new(p)?),
                None => None,
            },
        })
    }

    pub fn matches(&self, notification: &Notification) -> bool {
        if let Some(ref pattern) = self.pattern {
            let text = format!(
                "{} {} {} {}",
                notification.repo_full_name(),
                notification.title(),
                notification.notification_type(),
                notification.reason_enum()
            );
            pattern.is_match(&text)
        } else {
            true
        }
    }
}
