use std::collections::{HashMap, VecDeque};

use crate::app::App;
use crate::types::*;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use fieldx_plus::fx_plus;

#[fx_plus(agent(App, unwrap(error(anyhow::Error, App::app_is_gone()))), sync)]
pub(crate) struct Stats {
    #[fieldx(lock, set, get, get_mut, default(Vec::new()))]
    records: Vec<StatRecord>,

    /// The window size for the stats ticker.
    #[fieldx(lock, set, get(copy), default(60))]
    window: usize,

    #[fieldx(lock, get_mut, default(HashMap::new()))]
    per_second_rate: HashMap<DateTime<Utc>, usize>,

    /// Errors per current window
    #[fieldx(lock, get_mut, default(0))]
    errors: usize,

    /// Infos per current window
    #[fieldx(lock, get_mut, default(0))]
    infos: usize,

    /// Debugs per current window
    #[fieldx(lock, get_mut, default(0))]
    debugs: usize,

    #[fieldx(lock, get_mut, default(HashMap::new()))]
    err_msg_count: HashMap<String, usize>,
}

impl Stats {
    pub(crate) fn push_record(&self, rec: StatRecord) {
        let now = Utc::now();
        let current_rate = *self
            .per_second_rate_mut()
            .entry(now)
            .and_modify(|e| *e += 1)
            .or_insert(1);

        match rec.level() {
            Level::ERROR => {
                *self.errors_mut() += 1;
                self.err_msg_count_mut()
                    .entry(rec.message().to_owned())
                    .and_modify(|e| *e += 1)
                    .or_insert(1);
            }
            Level::INFO => {
                *self.infos_mut() += 1;
            }
            Level::DEBUG => {
                *self.debugs_mut() += 1;
            }
        }

        let rate = self.records().len() / self.window();
        if rate > 2500 {
            let cur_window = self.window();
            if cur_window > 15 {
                self.set_window(cur_window / 2);
            }
        } else if rate < 600 {
            let cur_window = self.window();
            if cur_window < 120 {
                // Increase gradually
                self.set_window(cur_window + 10);
            }
        }

        self.records_mut().push(rec);
    }

    fn clean_records(&self) {
        let records = self.records_mut();
        let oldest = Utc::now() - Duration::new(self.window() as i64, 0).unwrap();
        let new_records = records.iter().filter(|rec| {
            if rec.logged() < oldest {
                match rec.level() {
                    Level::ERROR => {
                        *self.errors_mut() -= 1;
                        let msg = rec.message().to_owned();
                        self.err_msg_count_mut().entry(msg).and_modify(|e| *e -= 1);
                    }
                    Level::INFO => *self.infos_mut() -= 1,
                    Level::DEBUG => *self.debugs_mut() -= 1,
                }
                false
            } else {
                true
            }
        });
    }

    pub(crate) async fn ticker(&self) -> Result<()> {
        loop {
            let rec = self.records_mut().pop();
            if let Some(rec) = rec {
                println!("{:?}", rec);
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        Ok(())
    }
}
