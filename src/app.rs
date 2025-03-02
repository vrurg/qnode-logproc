use std::sync::Arc;

use anyhow::Result;
use console::Term;
use fieldx::fxstruct;
use fieldx_plus::{agent_build, fx_plus};
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::JoinSet,
};

use crate::{reader::Reader, stats::Stats, types::LineMessage};

#[fxstruct(sync, no_new)]
pub(crate) struct Channel {
    #[fieldx(get(clone))]
    tx: Arc<UnboundedSender<LineMessage>>,
    #[fieldx(lock, get_mut("rx"))]
    rx: UnboundedReceiver<LineMessage>,
}

impl Channel {
    pub(crate) fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            tx: Arc::new(tx),
            rx: rx.into(),
        }
    }
}

#[fx_plus(app, sync, fallible(off, error(anyhow::Error)))]
pub(crate) struct App {
    #[fieldx(lock, get_mut(private))]
    task_set: JoinSet<()>,

    #[fieldx(lazy, fallible)]
    reader: crate::reader::Reader,

    #[fieldx(lazy, fallible)]
    parser: crate::parser::Parser,

    #[fieldx(lazy, fallible)]
    stats: Arc<crate::stats::Stats>,

    #[fieldx(lazy, private)]
    channel: Channel,

    #[fieldx(lazy, get)]
    term: console::Term,
}

impl App {
    pub async fn run() -> Result<()> {
        let app = App::new();

        let task_app = app.clone();
        tokio::spawn(async move {
            while let Err(err) = task_app.launch().await {
                task_app.task_set_mut().abort_all();
                eprintln!("App::launch failed, retrying; the error was: {:?}", err);
            }
        });

        app.ctrl_c().await?;

        Ok(())
    }

    async fn ctrl_c(&self) -> Result<()> {
        tokio::signal::ctrl_c().await?;
        println!("Ctrl-C received, shutting down");
        self.stats()?.shutdown();
        eprintln!("Abort all tasks");
        self.task_set_mut().abort_all();
        self.term().show_cursor()?;
        Ok(())
    }

    async fn launch(&self) -> Result<()> {
        // This is a feature of fieldx_plus, produces another copy of Arc-wrapped self.
        let myself = self.myself().unwrap();
        self.task_set_mut().spawn(async move {
            // This would fail only and only if reader builder fails. So, it's dev-time problem.
            let reader = myself.reader().unwrap();
            while let Err(err) = reader.start(myself.channel().tx()).await {
                eprintln!("Reader::start failed, retrying; the error was: {:?}", err);
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            eprintln!("Reader done.");
        });

        let myself = self.myself().unwrap();
        self.task_set_mut().spawn(async move {
            // This would fail only and only if analyzer builder fails. So, it's dev-time problem.
            let parser = myself.parser().unwrap();
            while let Err(err) = parser.start(&mut *myself.channel().rx()).await {
                eprintln!("Parser::start failed, retrying; the error was: {:?}", err);
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            eprintln!("Parser done.")
        });

        let myself = self.myself().unwrap();
        self.task_set_mut().spawn(async move {
            // This would fail only and only if analyzer builder fails. So, it's dev-time problem.
            let stats = myself.stats().unwrap();
            while let Err(err) = stats.start().await {
                eprintln!("Stats::start failed, retrying; the error was: {:?}", err);
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            eprintln!("Stats done.");
        });

        Ok(())
    }

    // See the `reader`, its `fieldx` `lazy` parameter above.
    fn build_reader(&self) -> Result<crate::reader::Reader> {
        agent_build!(self, Reader).map_err(|e| anyhow::anyhow!("Failed to build Reader: {:?}", e))
    }

    // See the `analyzer`, its `fieldx` `lazy` parameter above.
    fn build_parser(&self) -> Result<crate::parser::Parser> {
        agent_build!(self, crate::parser::Parser).map_err(|e| anyhow::anyhow!("Failed to build Parser: {:?}", e))
    }

    fn build_stats(&self) -> Result<Arc<Stats>> {
        agent_build!(self, Stats).map_err(|e| anyhow::anyhow!("Failed to build Stats: {:?}", e))
    }

    fn build_channel(&self) -> Channel {
        Channel::new()
    }

    fn build_term(&self) -> Term {
        Term::buffered_stdout()
    }

    // This is a universal error message if  app object was destroyed but an agent object remains alive and requesting
    // the app object.
    pub(crate) fn app_is_gone() -> anyhow::Error {
        anyhow::anyhow!("App object is gone while requested")
    }
}
