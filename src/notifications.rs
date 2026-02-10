use crate::api::GitHubClient;
use crate::error::Result;
use crate::models::Notification;

#[derive(Debug, Clone, Copy)]
pub struct NotificationFetchOptions {
    pub show_all: bool,
    pub participating: bool,
    pub max_notifications: Option<usize>,
    pub per_page: usize,
}

impl NotificationFetchOptions {
    pub fn effective_per_page(self) -> usize {
        let max_notifications = self.max_notifications.unwrap_or(usize::MAX);
        let per_page = self.per_page.max(1);
        per_page.min(max_notifications.max(1))
    }
}

pub fn fetch_notifications(
    client: &GitHubClient,
    options: NotificationFetchOptions,
) -> Result<Vec<Notification>> {
    NotificationFetcher::new(client, options).fetch()
}

struct NotificationFetcher<'a> {
    client: &'a GitHubClient,
    options: NotificationFetchOptions,
}

impl<'a> NotificationFetcher<'a> {
    fn new(client: &'a GitHubClient, options: NotificationFetchOptions) -> Self {
        Self { client, options }
    }

    fn fetch(&self) -> Result<Vec<Notification>> {
        let max_notifications = self.options.max_notifications.unwrap_or(usize::MAX);
        if max_notifications == 0 {
            return Ok(Vec::new());
        }

        let per_page = self.options.effective_per_page();
        let mut all_notifications = Vec::new();

        for page in 1.. {
            let notifications = self.client.get_notifications(
                self.options.show_all,
                self.options.participating,
                Some(per_page),
                Some(page),
            )?;

            if notifications.is_empty() {
                break;
            }

            let remaining = max_notifications.saturating_sub(all_notifications.len());
            if remaining == 0 {
                break;
            }

            all_notifications.extend(notifications.into_iter().take(remaining));
        }

        Ok(all_notifications)
    }
}
