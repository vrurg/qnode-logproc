use crate::{
    app::App,
    types::{Level, LineMessage, StatErrRecord, StatErrType, StatOKRecord, StatRecord},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use fieldx_plus::fx_plus;
use once_cell::sync::Lazy;
use regex::Regex;
use tokio::sync::mpsc::UnboundedReceiver;

static LINE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\[(?<dt>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)\]\s+(?<level>INFO|ERROR|DEBUG)\s+-\s+IP:(?<ip>\S+)\s+(?:Error \d+ -\s+)?(?<msg>.*)$")
        .unwrap()
});

#[fx_plus(agent(App, unwrap(error(anyhow::Error, App::app_is_gone()))), sync)]
pub(crate) struct Parser {}

impl Parser {
    pub(crate) async fn start(&self, rx: &mut UnboundedReceiver<LineMessage>) -> Result<()> {
        let app = self.app()?;
        // let reader = app.reader()?;

        loop {
            let line = rx.recv().await;
            if let Some(line) = line {
                self.parse_line(line).await?;
            }
            else {
                app.stats()?.stop()?;
                break;
            }
        }

        Ok(())
    }

    async fn parse_line(&self, line_msg: LineMessage) -> Result<()> {
        let app = self.app()?;
        if let Some(captures) = LINE_RE.captures(line_msg.line()) {
            let dt: DateTime<Utc> = captures.name("dt").unwrap().as_str().parse()?;
            let level: Level = captures.name("level").unwrap().as_str().parse()?;
            let msg = captures.name("msg").unwrap().as_str().to_string();

            app.stats()?.push_record(
                StatOKRecord::builder()
                    .received_millis(line_msg.recv_time_millis())
                    .logged_millis(dt.timestamp_millis())
                    .level(level)
                    .message(msg)
                    .build()?,
            )?;
        }
        else {
            app.stats()?.push_record(StatRecord::Err(
                StatErrRecord::builder()
                    .received_millis(line_msg.recv_time_millis())
                    .error_type(StatErrType::Malformed)
                    .line(line_msg.line().to_string())
                    .build()?,
            ))?;
        }

        Ok(())
    }
}
