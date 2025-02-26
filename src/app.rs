use anyhow::Result;
use fieldx_plus::{agent_build, fx_plus};
use tokio::task::JoinSet;

use crate::stats::Stats;

#[fx_plus(app, sync, get, fallible(off, error(anyhow::Error)))]
pub(crate) struct App {
    #[fieldx(lock, get_mut(private))]
    task_set: JoinSet<()>,

    #[fieldx(lazy, fallible)]
    reader: crate::reader::Reader,

    #[fieldx(lazy, fallible)]
    parser: crate::parser::Parser,

    #[fieldx(lazy, fallible)]
    stats: crate::stats::Stats,
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
        // Just for now. May require more graceful shutdown in the future.
        self.task_set_mut().abort_all();
        Ok(())
    }

    async fn launch(&self) -> Result<()> {
        // This is a feature of fieldx_plus, produces another copy of Arc-wrapped self.
        let myself = self.myself().unwrap();
        self.task_set_mut().spawn(async move {
            // This would fail only and only if reader builder fails. So, it's dev-time problem.
            let reader = myself.reader().unwrap();
            while let Err(err) = reader.start().await {
                eprintln!("Reader::start failed, retrying; the error was: {:?}", err);
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        });

        let myself = self.myself().unwrap();
        self.task_set_mut().spawn(async move {
            // This would fail only and only if analyzer builder fails. So, it's dev-time problem.
            let parser = myself.parser().unwrap();
            while let Err(err) = parser.start().await {
                eprintln!("Analyzer::start failed, retrying; the error was: {:?}", err);
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        });

        Ok(())
    }

    // See the `reader`, its `fieldx` `lazy` parameter above.
    fn build_reader(&self) -> Result<crate::reader::Reader> {
        agent_build!(self, crate::reader::Reader).map_err(|e| anyhow::anyhow!("Failed to build Reader: {:?}", e))
    }

    // See the `analyzer`, its `fieldx` `lazy` parameter above.
    fn build_parser(&self) -> Result<crate::parser::Parser> {
        agent_build!(self, crate::parser::Parser).map_err(|e| anyhow::anyhow!("Failed to build Analyzer: {:?}", e))
    }

    fn build_stats(&self) -> Result<Stats> {
        agent_build!(self, Stats).map_err(|e| anyhow::anyhow!("Failed to build Stats: {:?}", e))
    }

    // This is a universal error message if  app object was destroyed but an agent object remains alive and requesting
    // the app object.
    pub(crate) fn app_is_gone() -> anyhow::Error {
        anyhow::anyhow!("App object is gone while requested")
    }
}
