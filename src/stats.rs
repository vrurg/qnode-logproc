use core::f64;
use std::{
    cmp::Ordering,
    collections::{HashMap, VecDeque},
};

use crate::{app::App, types::*};
use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use fieldx::fxstruct;
use fieldx_plus::fx_plus;
use tokio::sync::mpsc::UnboundedSender;

// In seconds
const MSG_ERROR_WINDOW: i64 = 15000;
// Window bounds in seconds
const MIN_WINDOW: usize = 30;
const MAX_WINDOW: usize = 120;

#[derive(Clone, Copy)]
enum Act {
    Inc = 1,
    Dec = -1,
}

#[fxstruct(no_new, default)]
struct StatsSnapshot {
    entries:            i64,
    // In milliseconds
    collected_interval: i64,
    rate:               f64,
    peak_rate:          f64,
    error_rate:         f32,

    errors:    i64,
    infos:     i64,
    debugs:    i64,
    malformed: i64,

    /// Map a message ID to the number of times it has been seen
    error_msg_counts: HashMap<u64, i64>,

    /// For each second, map a message ID to the number of times it has been seen in that second
    error_msg_per_sec: HashMap<i64, HashMap<u64, i64>>,

    /// Map a message ID to its weight
    error_msg_rates: HashMap<u64, f64>,

    /// Measure in milliseconds
    #[fieldx(default(60000))]
    window: usize,

    #[fieldx(default(60000))]
    previous_window: usize,

    /// List of timestamps in milliseconds of messages received in the last second
    last_second_received: VecDeque<i64>,
}

impl StatsSnapshot {
    fn count_inner_rec(&mut self, rec: InnerRecord, act: Act) -> InnerRecord {
        match &rec {
            InnerRecord::OK(ok) => match ok.level {
                Level::ERROR => {
                    self.errors += act as i64;
                    self.error_msg_counts
                        .entry(ok.msg_id)
                        .and_modify(|count| *count += act as i64)
                        .or_insert(act as i64);

                    let log_time = rec.log_timestamp();
                    let cnt = *self
                        .error_msg_per_sec
                        .entry(log_time)
                        .or_insert_with(HashMap::new)
                        .entry(ok.msg_id)
                        .and_modify(|count| *count += act as i64)
                        .or_insert(act as i64);
                    // Clean up empty entries.
                    // Since log times are not guaranteed to be monotonically increasing and can fall out
                    // of the current time window, we need to check if the count is zero here for better performance.
                    // Otherwise, it would be necessary to iterate over all entries in the cleanup_and_adjust body.
                    if cnt == 0 {
                        self.error_msg_per_sec.get_mut(&log_time).unwrap().remove(&ok.msg_id);
                        if self.error_msg_per_sec[&log_time].is_empty() {
                            self.error_msg_per_sec.remove(&log_time);
                        }
                    }
                }
                Level::INFO => {
                    self.infos += act as i64;
                }
                Level::DEBUG => {
                    self.debugs += act as i64;
                }
            },
            InnerRecord::Err(err) => match err.err_type {
                StatErrType::Malformed => {
                    self.malformed += act as i64;
                }
            },
        }

        rec
    }
}

enum InnerRecord {
    OK(InnerOKRecord),
    Err(InnerErrRecord),
}

impl InnerRecord {
    fn recv_timestamp_millis(&self) -> i64 {
        match self {
            Self::OK(ok) => ok.received_millis,
            Self::Err(err) => err.received_millis,
        }
    }

    #[allow(dead_code)]
    fn recv_timestamp(&self) -> i64 {
        self.recv_timestamp_millis() / 1000
    }

    fn log_timestamp_millis(&self) -> i64 {
        match self {
            Self::OK(ok) => ok.logged_millis,
            Self::Err(err) => err.received_millis,
        }
    }

    fn log_timestamp(&self) -> i64 {
        self.log_timestamp_millis() / 1000
    }
}

struct InnerOKRecord {
    received_millis: i64,
    logged_millis:   i64,
    level:           Level,
    msg_id:          u64,
}

struct InnerErrRecord {
    received_millis: i64,
    err_type:        StatErrType,
}

impl StatsSnapshot {
    fn refresh_last_second(&mut self, ts: Option<i64>) {
        if let Some(ts) = ts {
            self.last_second_received.push_front(ts);
        }
        let now = Utc::now().timestamp_millis();
        while let Some(last) = self.last_second_received.back() {
            if now - last > 1000 {
                self.last_second_received.pop_back();
            }
            else {
                break;
            }
        }
    }
}

#[fx_plus(
    agent(App, unwrap(error(anyhow::Error, App::app_is_gone()))),
    sync,
    rc,
    fallible(off, error(anyhow::Error))
)]
pub(crate) struct Stats {
    #[fieldx(lock, private, get, get_mut, default(VecDeque::new()))]
    records: VecDeque<InnerRecord>,

    /// List of all distinct log messages encountered
    #[fieldx(private, lock, get, get_mut, default(Vec::new()))]
    msgs: Vec<String>,

    /// Map a log message to its index in `msgs`.
    #[fieldx(private, lock, get_mut, default(HashMap::new()))]
    msg_idx: HashMap<String, u64>,

    #[fieldx(lock, private, get_mut, builder(off))]
    stat: StatsSnapshot,

    #[fieldx(lazy, fallible, clearer, private, get)]
    tx: UnboundedSender<StatRecord>,
}

impl Stats {
    pub(crate) async fn start(&self) -> Result<()> {
        let app = self.app()?;

        loop {
            let term = app.term();
            term.clear_screen()?;
            term.move_cursor_to(0, 0)?;

            let now = Local::now();

            if self.records().is_empty() {
                term.write_line(&format!("{} No records yet.", now.format("%Y-%m-%d %H:%M:%S%.3f")))?;
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                continue;
            }

            // Use lexical scope to localize stat_snapshot lock.
            {
                let mut stat_snapshot = self.stat_mut();
                self.cleanup_and_adjust(&mut *stat_snapshot);
                self.print_report(now, &stat_snapshot)?;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        }
    }

    pub fn shutdown(&self) {
        self.clear_tx();
    }

    fn print_report(&self, now: DateTime<Local>, stat_snapshot: &StatsSnapshot) -> Result<()> {
        let app = self.app()?;
        let term = app.term();
        term.clear_screen()?;
        term.move_cursor_to(0, 0)?;
        term.write_line(&format!("Stats as of {}", now.format("%Y-%m-%d %H:%M:%S%.3f")))?;
        term.write_line(&std::iter::repeat('-').take(80).collect::<String>())?;
        term.write_line(&format!(
            "Entries: {} per {:.2} seconds (window: {}sec)",
            stat_snapshot.entries,
            stat_snapshot.collected_interval as f64 / 1000.0,
            stat_snapshot.window / 1000
        ))?;
        term.write_line(&format!(
            "Current rate: {:.2} entries/sec",
            stat_snapshot.last_second_received.len()
        ))?;
        term.write_line(&format!("Rate        : {:.2} entries/sec", stat_snapshot.rate))?;
        term.write_line(&format!("Peak rate   : {:.2} entries/sec", stat_snapshot.peak_rate))?;
        term.write_line("")?;
        term.write_line(&format!(
            "Errors: {:.2}% ({} entries); rate: {:.2} errors/sec",
            stat_snapshot.errors as f32 / stat_snapshot.entries as f32 * 100.0,
            stat_snapshot.errors,
            stat_snapshot.error_rate
        ))?;
        term.write_line(&format!(
            "Infos: {:.2}% ({} entries)",
            stat_snapshot.infos as f32 / stat_snapshot.entries as f32 * 100.0,
            stat_snapshot.infos
        ))?;
        term.write_line(&format!(
            "Debugs: {:.2}% ({} entries)",
            stat_snapshot.debugs as f32 / stat_snapshot.entries as f32 * 100.0,
            stat_snapshot.debugs
        ))?;
        term.write_line(&format!("Malformed: {}", stat_snapshot.malformed))?;
        term.write_line("")?;
        term.write_line("Top error messages:")?;

        let mut msgs = stat_snapshot.error_msg_counts.iter().collect::<Vec<_>>();
        msgs.sort_by(|a, b| b.1.cmp(a.1));

        for (pos, msg) in msgs.iter().take(3).enumerate() {
            term.write_line(&format!(
                "  {}. \"{}\" ({} entries)",
                pos + 1,
                self.msg_by_id(*msg.0),
                msg.1
            ))?;
        }

        term.write_line("")?;
        term.write_line("Trending messages:")?;

        let mut rates = stat_snapshot.error_msg_rates.iter().collect::<Vec<_>>();
        rates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        for rate in rates.iter().take(3) {
            term.write_line(&format!("  \"{}\" (rate: {:.2})", self.msg_by_id(*rate.0), rate.1))?;
        }

        term.write_line("")?;
        term.write_line("Insights:")?;
        term.write_line(&format!(
            "Error messages per second table size: {}",
            stat_snapshot.error_msg_per_sec.len()
        ))?;

        term.write_line(&std::iter::repeat('-').take(80).collect::<String>())?;
        term.write_line("Ctrl-C to stop.")?;

        term.flush()?;

        Ok(())
    }

    pub fn msg_id(&self, msg: &str) -> u64 {
        let mut msg_idx = self.msg_idx_mut();
        if let Some(id) = msg_idx.get(msg) {
            return *id;
        }

        let mut msgs = self.msgs_mut();
        let new_id = msgs.len() as u64;
        msgs.push(msg.to_owned());
        msg_idx.insert(msg.to_owned(), new_id);

        new_id
    }

    pub fn msg_by_id(&self, id: u64) -> String {
        self.msgs()
            .get(id as usize)
            .map_or("N/A".to_string(), |msg| msg.clone())
    }

    pub(crate) fn push_record<S: Into<StatRecord>>(&self, rec: S) -> Result<()> {
        self.tx()?.send(rec.into())?;
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        self.tx()?.send(StatRecord::Stop).unwrap();
        Ok(())
    }

    fn recalc_weights(&self, stat_snapshot: &mut StatsSnapshot, now: i64) {
        if self.records().len() == 0 {
            return;
        }
        let window_size = (self.records().front().unwrap().log_timestamp_millis()
            - self.records().back().unwrap().log_timestamp_millis())
        .max(MSG_ERROR_WINDOW);

        // We need at least 2 seconds of data to calculate the weights
        if window_size < 2000 {
            return;
        }

        let mut seconds = stat_snapshot.error_msg_per_sec.keys().copied().collect::<Vec<_>>();
        seconds.sort_by(|a, b| b.cmp(a));

        // Group by MSG_ERROR_WINDOW seconds from now. We need the last two groups only.
        // index 0 is for newer, 1 is for older
        let mut grouped = vec![HashMap::new(); 2];
        let base_time_millis = vec![(now - window_size / 2), now];

        for sec in seconds.iter().copied() {
            let msec = sec * 1000;

            let group_idx = ((now - msec) * 2 / window_size) as usize;

            // Too old messages are not interesting
            if group_idx > 1 {
                break;
            }

            let counts = stat_snapshot.error_msg_per_sec.get(&sec).unwrap();

            for (msg_id, count) in counts.iter() {
                let weight = grouped[group_idx].entry(*msg_id).or_insert(0.0);
                *weight += f64::consts::E.powf((base_time_millis[group_idx] - msec) as f64 / window_size as f64)
                    * (*count as f64);
            }
        }

        for msg_id in stat_snapshot.error_msg_counts.keys() {
            stat_snapshot.error_msg_rates.insert(
                *msg_id,
                if let Some(older) = grouped[1].get(msg_id) {
                    grouped[0].get(msg_id).unwrap_or(&0.0) / *older
                }
                else {
                    0.0
                },
            );
        }
    }

    fn cleanup_and_adjust(&self, stat_snapshot: &mut StatsSnapshot) {
        let now = Utc::now().timestamp_millis();
        let oldest = now - stat_snapshot.window as i64;

        let mut recalc = true;

        while recalc {
            recalc = false;

            // records write lock scope
            {
                let mut records = self.records_mut();
                stat_snapshot.entries = records.len() as i64;

                while let Some(rec) = records.back() {
                    if rec.recv_timestamp_millis() < oldest {
                        stat_snapshot.count_inner_rec(records.pop_back().unwrap(), Act::Dec);
                    }
                    else {
                        break;
                    }
                }

                stat_snapshot.collected_interval = records.front().map_or(0, |r| r.recv_timestamp_millis())
                    - records.back().map_or(0, |r| r.recv_timestamp_millis());
            }
            // We need per second, not per millisecond
            if stat_snapshot.collected_interval > 100 {
                stat_snapshot.rate = stat_snapshot.entries as f64 / (stat_snapshot.collected_interval as f64 / 1000.0);
            }
            if stat_snapshot.collected_interval >= 1000 {
                // Let the stats stabilize first
                stat_snapshot.peak_rate = stat_snapshot.rate.max(stat_snapshot.peak_rate);
            }
            stat_snapshot.error_rate = stat_snapshot.errors as f32 / stat_snapshot.entries as f32;

            self.recalc_weights(&mut *stat_snapshot, now);

            // Adjust window if necessary. The technical spec requires, say, 30 secs window for 2,500 entries/sec.
            // Let's make it weighted dynamic decision. So, 2500*30 = 75,000 entries per window. Rust can do much better,
            // let's round it to 100k and try keeping the records queue size around that.
            if stat_snapshot.rate > 0.0 {
                // Calculate expected buffer size
                let expected_buffer_size = stat_snapshot.rate * (stat_snapshot.window as f64 / 1000.0);
                if expected_buffer_size > 100_000.0 || expected_buffer_size < 75_000.0 {
                    let new_window = ((100_000.0 / stat_snapshot.rate) as usize)
                        .min(MIN_WINDOW)
                        .max(MAX_WINDOW)
                        * 1000;
                    if new_window != stat_snapshot.window {
                        stat_snapshot.previous_window = stat_snapshot.window;
                        stat_snapshot.window = new_window;
                        recalc = true;
                        eprintln!("NEED RECALC");
                    }
                }
            }
        }
    }

    fn process_ok(&self, rec: StatOKRecord) {
        let mut stat_snapshot = self.stat_mut();

        // Refresh the last second list so we know the current rate
        stat_snapshot.refresh_last_second(Some(rec.received_millis()));

        let msg_id = self.msg_id(&rec.message());
        let inner_rec = InnerOKRecord {
            received_millis: rec.received_millis(),
            logged_millis: rec.logged_millis(),
            level: rec.level(),
            msg_id,
        };

        self.records_mut()
            .push_front(stat_snapshot.count_inner_rec(InnerRecord::OK(inner_rec), Act::Inc));

        self.cleanup_and_adjust(&mut *stat_snapshot);
    }

    fn process_err(&self, rec: StatErrRecord) {
        let mut stat_snapshot = self.stat_mut();

        let inner_err = InnerErrRecord {
            received_millis: rec.received_millis(),
            err_type:        rec.error_type(),
        };

        self.records_mut()
            .push_front(stat_snapshot.count_inner_rec(InnerRecord::Err(inner_err), Act::Inc));
    }

    fn process_incoming(&self, mut rx: tokio::sync::mpsc::UnboundedReceiver<StatRecord>) {
        while let Some(rec) = rx.blocking_recv() {
            match rec {
                StatRecord::OK(ok) => {
                    self.process_ok(ok);
                }
                StatRecord::Err(err) => {
                    self.process_err(err);
                }
                StatRecord::Stop => {
                    break;
                }
            }
        }
        println!("Done processing incoming...");
    }

    fn build_tx(&self) -> Result<UnboundedSender<StatRecord>> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<StatRecord>();

        let myself = self.myself().unwrap();
        self.app()?.task_set_mut().spawn_blocking(move || {
            myself.process_incoming(rx);
        });

        Ok(tx)
    }
}
