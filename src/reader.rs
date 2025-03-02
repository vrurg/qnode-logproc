use std::sync::Arc;

use crate::{app::App, types::LineMessage};
use anyhow::Result;
use fieldx_plus::fx_plus;
use tokio::{
    io::{self, AsyncBufReadExt, BufReader},
    sync::mpsc::UnboundedSender,
};

#[fx_plus(agent(App, unwrap(error(anyhow::Error, App::app_is_gone()))), sync)]
pub(crate) struct Reader {}

impl Reader {
    pub(crate) async fn start(&self, tx: Arc<UnboundedSender<LineMessage>>) -> Result<()> {
        let reader = BufReader::new(io::stdin());
        let mut lines = reader.lines();

        'read: loop {
            let line = match lines.next_line().await {
                Ok(Some(l)) => l,

                Ok(None) => break 'read,
                Err(err) => {
                    return Err(err.into());
                }
            };

            let line_msg = LineMessage::new(line, chrono::Utc::now().timestamp_millis());

            tx.send(line_msg)?;
        }

        Ok(())
    }
}
