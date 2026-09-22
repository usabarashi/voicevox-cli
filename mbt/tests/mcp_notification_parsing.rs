//! Model-based test for MCP notification parsing.
//!
//! The driver calls the **production** `parse_notification_message` with one
//! fixture per action and lets Quint Connect compare the observed outcome
//! against `modeling/quint/McpNotificationParsing.qnt`. This covers the
//! cancellation-notification routing that the request-parsing MBT does not.

use anyhow::Context;
use quint_connect::*;
use serde::Deserialize;
use serde_json::json;
use voicevox_cli::interface::mcp_server::protocol::{
    NotificationMethod, parse_notification_message,
};

/// Mirrors the spec's `Outcome` sum type.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum Outcome {
    Initialized,
    Cancelled,
    Unknown,
}

#[derive(Default)]
struct NotificationDriver {
    outcome: Option<Outcome>,
}

impl NotificationDriver {
    fn reset(&mut self) {
        // Mirrors the spec's `init`.
        self.outcome = Some(Outcome::Unknown);
    }

    fn parse(&mut self, raw: serde_json::Value) {
        self.outcome = Some(match parse_notification_message(raw).method {
            NotificationMethod::Initialized => Outcome::Initialized,
            NotificationMethod::Cancelled(_) => Outcome::Cancelled,
            NotificationMethod::Unknown => Outcome::Unknown,
        });
    }
}

impl State<NotificationDriver> for Outcome {
    fn from_driver(driver: &NotificationDriver) -> Result<Self> {
        driver.outcome.clone().context("outcome not set")
    }
}

impl Driver for NotificationDriver {
    type State = Outcome;

    fn config() -> Config {
        Config {
            state: &["outcome"],
            ..Config::default()
        }
    }

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.reset(),
            parseInitialized => self.parse(json!({ "method": "notifications/initialized" })),
            parseCancelled => self.parse(json!({
                "method": "notifications/cancelled",
                "params": { "requestId": 42, "reason": "ESC pressed" }
            })),
            parseCancelledWithoutParams => self.parse(json!({
                "method": "notifications/cancelled",
                "params": {}
            })),
            parseUnknown => self.parse(json!({ "method": "notifications/other" })),
            _ => (),
        })
    }
}

/// Random exploration of the notification-parsing outcomes.
#[quint_run(
    spec = "../modeling/quint/McpNotificationParsing.qnt",
    max_samples = 200,
    max_steps = 6
)]
fn mcp_notification_parsing_simulation() -> impl Driver {
    NotificationDriver::default()
}
