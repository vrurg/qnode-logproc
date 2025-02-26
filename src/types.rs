use chrono::{DateTime, Utc};
use fieldx::fxstruct;
use strum_macros::EnumString;

#[derive(Debug, Clone, Copy, EnumString)]
pub(crate) enum Level {
    INFO,
    ERROR,
    DEBUG,
}

#[derive(Debug, Clone)]
#[fxstruct(sync, no_new, builder, get)]
pub(crate) struct StatRecord {
    #[fieldx(get(copy))]
    received: DateTime<Utc>,
    #[fieldx(get(copy))]
    logged: DateTime<Utc>,
    #[fieldx(get(copy))]
    level: Level,
    message: String,
}
