use crate::ConfigVars;
use anyhow::{bail, Result};
use reqwest::{Client, StatusCode};
use serde_json::json;

pub struct SlackBot {
    url: String,
    http_client: Client,
    is_prod: bool,
}

impl SlackBot {
    pub fn new(configvars: ConfigVars) -> SlackBot {
        SlackBot {
            url: configvars.slack_url,
            http_client: Client::default(),
            is_prod: configvars.is_prod,
        }
    }

    pub async fn send_msg(&self, msg: &str) -> Result<()> {
        if !self.is_prod {
            return Ok(());
        }

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
