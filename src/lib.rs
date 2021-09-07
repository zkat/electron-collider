use std::env;
use std::path::PathBuf;

use collider_command::ColliderCommand;
use collider_command::{
    async_trait::async_trait,
    clap::{self, ArgMatches, Clap, FromArgMatches, IntoApp},
    collider_config::{ColliderConfig, ColliderConfigLayer, ColliderConfigOptions},
    log,
};
use collider_common::{
    directories::ProjectDirs,
    miette::{Context, IntoDiagnostic, Result},
};

#[derive(Debug, Clap)]
#[clap(
    author = "Kat Marchán <kzm@zkat.tech>",
    about = "Build and manage your Electron application.",
    version = clap::crate_version!(),
    setting = clap::AppSettings::ColoredHelp,
    setting = clap::AppSettings::DisableHelpSubcommand,
    setting = clap::AppSettings::DeriveDisplayOrder,
    setting = clap::AppSettings::InferSubcommands,
)]
pub struct Collider {
    #[clap(global = true, long = "root", about = "Package path to operate on.")]
    root: Option<PathBuf>,
    #[clap(global = true, about = "File to read configuration values from.", long)]
    config: Option<PathBuf>,
    #[clap(
        global = true,
        about = "Log output level (off, error, warn, info, debug, trace)",
        long,
        default_value = "warn"
    )]
    loglevel: log::LevelFilter,
    #[clap(global = true, about = "Disable all output", long, short = 'q')]
    quiet: bool,
    #[clap(global = true, long, about = "Format output as JSON.")]
    json: bool,
    #[clap(subcommand)]
    subcommand: ColliderCmd,
}

impl Collider {
    fn setup_logging(&self) -> std::result::Result<(), fern::InitError> {
        let fern = fern::Dispatch::new()
            .format(|out, message, record| {
                out.finish(format_args!(
                    "collider [{}][{}] {}",
                    record.level(),
                    record.target(),
                    message,
                ))
            })
            .chain(
                fern::Dispatch::new()
                    .level(if self.quiet {
                        log::LevelFilter::Off
                    } else {
                        self.loglevel
                    })
                    .chain(std::io::stderr()),
            );
        // TODO: later
        // if let Some(logfile) = ProjectDirs::from("", "", "collider")
        //     .map(|d| d.data_dir().to_owned().join(format!("collider-debug-{}.log", chrono::Local::now().to_rfc3339())))
        // {
        //     fern = fern.chain(
        //         fern::Dispatch::new()
        //         .level(log::LevelFilter::Trace)
        //         .chain(fern::log_file(logfile)?)
        //     )
        // }
        fern.apply()?;
        Ok(())
    }

    pub async fn load() -> Result<()> {
        let start = std::time::Instant::now();
        let clp = Collider::into_app();
        let matches = clp.get_matches();
        let mut collider = Collider::from_arg_matches(&matches);
        let cfg = if let Some(file) = &collider.config {
            ColliderConfigOptions::new()
                .global_config_file(Some(file.clone()))
                .load()?
        } else {
            ColliderConfigOptions::new()
                .global_config_file(
                    ProjectDirs::from("", "", "collider")
                        .map(|d| d.config_dir().to_owned().join("colliderrc.toml")),
                )
                .pkg_root(collider.root.clone())
                .load()?
        };
        collider.layer_config(&matches, &cfg)?;
        collider
            .setup_logging()
            .into_diagnostic()
            .context("Failed to set up logging")?;
        collider.execute().await?;
        log::info!("Ran in {}s", start.elapsed().as_millis() as f32 / 1000.0);
        Ok(())
    }
}

#[derive(Debug, Clap)]
pub enum ColliderCmd {
    #[clap(
        about = "Bisect the Electron version that caused a breakage.",
        setting = clap::AppSettings::ColoredHelp,
        setting = clap::AppSettings::DisableHelpSubcommand,
        setting = clap::AppSettings::DeriveDisplayOrder,
    )]
    Bisect(collider_cmd_bisect::BisectCmd),
    #[clap(
        about = "Scaffold a new Electron application based on a workload.",
        setting = clap::AppSettings::ColoredHelp,
        setting = clap::AppSettings::DisableHelpSubcommand,
        setting = clap::AppSettings::DeriveDisplayOrder,
    )]
    New(collider_cmd_new::NewCmd),
    #[clap(
        about = "Pack an application for release",
        setting = clap::AppSettings::ColoredHelp,
        setting = clap::AppSettings::DisableHelpSubcommand,
        setting = clap::AppSettings::DeriveDisplayOrder,
    )]
    Pack(collider_cmd_pack::PackCmd),
    #[clap(
        about = "Start your Electron application.",
        setting = clap::AppSettings::ColoredHelp,
        setting = clap::AppSettings::DisableHelpSubcommand,
        setting = clap::AppSettings::DeriveDisplayOrder,
    )]
    Start(collider_cmd_start::StartCmd),
}

#[async_trait]
impl ColliderCommand for Collider {
    async fn execute(self) -> Result<()> {
        log::info!("Running command: {:#?}", self.subcommand);
        use ColliderCmd::*;
        match self.subcommand {
            Bisect(cmd) => cmd.execute().await,
            New(cmd) => cmd.execute().await,
            Pack(cmd) => cmd.execute().await,
            Start(cmd) => cmd.execute().await,
        }
    }
}

impl ColliderConfigLayer for Collider {
    fn layer_config(&mut self, args: &ArgMatches, conf: &ColliderConfig) -> Result<()> {
        use ColliderCmd::*;
        let (cmd, match_name): (&mut dyn ColliderConfigLayer, &str) = match self.subcommand {
            Bisect(ref mut cmd) => (cmd, "bisect"),
            New(ref mut cmd) => (cmd, "new"),
            Pack(ref mut cmd) => (cmd, "pack"),
            Start(ref mut cmd) => (cmd, "start"),
        };
        cmd.layer_config(args.subcommand_matches(match_name).unwrap(), conf)
    }
}
