use crate::{
    app::App,
    types::{Level, StatRecord},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use fieldx_plus::fx_plus;
use once_cell::sync::Lazy;
use regex::Regex;

static LINE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\[(?<dt>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)\]\s+(?<level>INFO|ERROR|DEBUG)(?:\s+Error \d+ -)?\s+(?<msg>.*)$")
        .unwrap()
});

#[fx_plus(agent(App, unwrap(error(anyhow::Error, App::app_is_gone()))), sync)]
pub(crate) struct Parser {}

impl Parser {
    pub(crate) async fn start(&self) -> Result<()> {
        let app = self.app()?;
        // let reader = app.reader()?;

        loop {
            let line = app.reader()?.pull_line().await;
            if let Some(line) = line {
                self.parse_line(line).await?;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        }

        Ok(())
    }

    async fn parse_line(&self, line: String) -> Result<()> {
        let app = self.app()?;
        if let Some(captures) = LINE_RE.captures(&line) {
            eprintln!("MATCHED: {}", line);
            let dt: DateTime<Utc> = captures.name("dt").unwrap().as_str().parse()?;
            let level: Level = captures.name("level").unwrap().as_str().parse()?;
            let msg = captures.name("msg").unwrap().as_str().to_string();

            app.stats()?.push_record(
                StatRecord::builder()
                    .received(Utc::now())
                    .logged(dt)
                    .level(level)
                    .message(msg)
                    .build()?,
            );
        } else {
            eprintln!("NO MATCH: {}", line);
        }

        Ok(())
    }
}
