use anyhow::{bail, Result};
use reqwest::{Client, StatusCode};
use serde_json::json;
use std::env;

pub struct SlackBot {
    url: String,
    http_client: Client,
}

impl SlackBot {
    pub fn try_new() -> Result<SlackBot> {
        Ok(SlackBot {
            url: env::var("SLACK_URL")?,
            http_client: Client::default(),
        })
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
    use dotenv::dotenv;

    #[tokio::test]
    async fn test_can_send_message_to_slack() -> Result<()> {
        dotenv()?;

        let slack_bot = SlackBot::try_new()?;

        slack_bot.send_msg("hello from rust, again").await?;

        Ok(())
    }
}
