use std::collections::VecDeque;

use crate::app::App;
use fieldx_plus::fx_plus;
use tokio::io::{self, AsyncBufReadExt, BufReader};

#[fx_plus(agent(App, unwrap(error(anyhow::Error, App::app_is_gone()))), sync)]
pub(crate) struct Reader {
    #[fieldx(lock, get, get_mut, default(VecDeque::new()))]
    buf: VecDeque<String>,
}

impl Reader {
    pub(crate) async fn start(&self) -> io::Result<()> {
        let reader = BufReader::new(io::stdin());
        let mut lines = reader.lines();

        loop {
            while let Some(line) = lines.next_line().await? {
                // eprintln!("l: {}", line);
                self.buf_mut().push_back(line);
                self.adjust_capacity();
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        }

        Ok(())
    }

    pub(crate) async fn pull_line(&self) -> Option<String> {
        let mut bm = self.buf_mut();
        let line = bm.pop_front();
        line
    }

    fn adjust_capacity(&self) {
        // Check if the line buffer capacity needs to change to avoid unnecessary allocations or extra memory
        // consumption.  Though, considering that String is just a pointer to a heap-allocated buffer, it might
        // be better to just let the buffer grow.
        let (capacity, len) = {
            let buf = self.buf();
            (buf.capacity(), buf.len())
        };
        if len > (capacity / 2) {
            self.buf_mut().reserve(capacity * 2);
        } else if len < (capacity / 4) && capacity > 500 {
            self.buf_mut().shrink_to(capacity / 2);
        }
    }
}
