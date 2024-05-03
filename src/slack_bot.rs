use anyhow::{bail, Result};
use reqwest::{Client, StatusCode};
use serde_json::json;

pub struct SlackBot {
    url: String,
    http_client: Client,
}

impl SlackBot {
    pub fn new(url: String) -> SlackBot {
        SlackBot {
            url,
            http_client: Client::default(),
        }
    }

    pub async fn send_msg(&self, msg: &str) -> Result<()> {
        let data = json!({
            "text": msg
        });
        let data = serde_json::to_string(&data)?;

        let response = self.http_client.post(&self.url).body(data).send().await?;

        if response.status() == StatusCode::OK {
            return Ok(());
        }

        bail!("notification to slack failed")
    }
}

#[cfg(test)]
mod bot_test {
    use super::*;

    #[tokio::test]
    async fn test_can_send_message_to_slack() -> Result<()> {
        let url = dotenv::var("SLACK_URL")?;

        let slack_bot = SlackBot::new(url);

        slack_bot.send_msg("hello from rust, again").await?;

        Ok(())
    }
}
