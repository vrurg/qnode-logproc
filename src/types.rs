#![allow(dead_code)]
use fieldx::fxstruct;
use strum_macros::EnumString;

#[derive(Debug, Clone, Copy, EnumString)]
pub(crate) enum Level {
    INFO,
    ERROR,
    DEBUG,
}

#[derive(Debug, Clone)]
pub(crate) enum StatRecord {
    OK(StatOKRecord),
    Err(StatErrRecord),
    Stop,
}

impl StatRecord {
    pub(crate) fn received_millis(&self) -> i64 {
        match self {
            Self::OK(ok) => ok.received_millis(),
            Self::Err(err) => err.received_millis(),
            Self::Stop => -1,
        }
    }

    pub(crate) fn received(&self) -> i64 {
        self.received_millis() / 1000
    }
}

impl From<StatOKRecord> for StatRecord {
    fn from(ok: StatOKRecord) -> Self {
        Self::OK(ok)
    }
}

impl From<StatErrRecord> for StatRecord {
    fn from(err: StatErrRecord) -> Self {
        Self::Err(err)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum StatErrType {
    Malformed,
}

#[derive(Debug, Clone)]
#[fxstruct(sync, no_new, builder, get)]
pub(crate) struct StatOKRecord {
    /// When the incoming message was received by the reader, in milliseconds
    #[fieldx(get(copy))]
    received_millis: i64,
    /// When the message was generated, in milliseconds
    #[fieldx(get(copy))]
    logged_millis:   i64,
    #[fieldx(get(copy))]
    level:           Level,
    message:         String,
}

#[derive(Debug, Clone)]
#[fxstruct(sync, no_new, builder, get)]
pub(crate) struct StatErrRecord {
    /// When the incoming message was received by the reader, in milliseconds
    #[fieldx(get(copy))]
    received_millis: i64,
    #[fieldx(get(copy))]
    error_type:      StatErrType,
    /// If there is a line associated with the error, it is stored here
    #[fieldx(optional)]
    line:            String,
}

#[fxstruct(get, no_new)]
pub(crate) struct LineMessage {
    line:             String,
    #[fieldx(get(copy))]
    recv_time_millis: i64,
}

impl LineMessage {
    pub(crate) fn new(line: String, recv_time_millis: i64) -> Self {
        Self { line, recv_time_millis }
    }

    pub(crate) fn recv_time(&self) -> i64 {
        self.recv_time_millis() / 1000
    }
}
