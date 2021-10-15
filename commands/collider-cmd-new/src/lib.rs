use std::path::{Path, PathBuf};

use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    dialoguer::{Input, theme::ColorfulTheme},
    tracing, ColliderCommand,
};

use collider_common::{
    miette::{self, Context, IntoDiagnostic, Result},
    smol::{fs, prelude::*, process::Command},
};

// use collider_electron::ElectronOpts;

#[derive(Debug, Clap, ColliderConfigLayer)]
pub struct NewCmd {
    #[clap(about = "Path to create new Electron application in.")]
    path: PathBuf,
    #[clap(
        long,
        short = 't',
        default_value = "vanilla",
        about = "Template to use when scaffolding a new application."
    )]
    template: String,
    #[clap(from_global)]
    verbosity: tracing::Level,
    #[clap(from_global)]
    quiet: bool,
    #[clap(from_global)]
    json: bool,
}

#[async_trait]
impl ColliderCommand for NewCmd {
    async fn execute(self) -> Result<()> {
        let name : String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("What's your project name?: ")
            .with_initial_text("")
            .default("new-collider-project".into())
            .interact_text().into_diagnostic()?;
        let proj_path = &self.path.join(&name);

        self.create_new_dir(&proj_path).await?;
        println!(
            "Created a new Electron project - {} - at {}",
            name,
            proj_path.display(),
        );

        let initialize : String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Would you like to npm install and build your project now? (yes/no)")
            .with_initial_text("")
            .interact_text().into_diagnostic()?;
        if initialize == "yes" {
            self.init(&proj_path).await?;
        }
        println!("{} is ready for development ✨", &name);
        Ok(())
    }
}

impl NewCmd {
    async fn create_new_dir(&self, dir: &PathBuf) -> Result<()> {
        fs::create_dir(&dir).await.into_diagnostic()?;

        // TODO: use walkdir here and preload some of the files with author
        // project names and license data, etc
        let template_path = Path::new("./commands/collider-cmd-new/templates/quick-start");
        let mut entries = fs::read_dir(template_path).await.into_diagnostic()?;
        while let Some(res) = entries.next().await {
            let entry = res.into_diagnostic()?;
            let path = entry.path();
            match path.file_name() {
                Some(filename) => {
                    let dest_path = &dir.join(filename);
                    fs::copy(&path, &dest_path).await.into_diagnostic()?;
                }
                None => {
                    println!("failed: {:?}", path);
                }
            }
        }
        Ok(())
    }

    async fn init(&self, proj_dir: &PathBuf) -> Result<()> {
        // dev: npm i and npm run start in the new project directory
        let npm_path = which::which("npm").into_diagnostic().context(
            "Failed to find npm command while packaging project. NPM/npx are required by collider.",
        )?;

        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.arg("/c");
            cmd.arg(npm_path);
            cmd
        } else {
            Command::new(npm_path)
        };

        let install_status = cmd
            .arg("install")
            .current_dir(proj_dir)
            .status()
            .await
            .into_diagnostic()
            .context("Failed to spawn NPM itself.")?;

        let start_status = cmd
            .arg("run")
            .arg("start")
            .current_dir(proj_dir)
            .status()
            .await
            .into_diagnostic()
            .context("Failed to start project with npm run start.")?;

        if !install_status.success() || !start_status.success() {
            miette::bail!("Initializing project failed")
        }
        // run the electron binary instead?
        // let opts = ElectronOpts::new();
        // let electron = opts.ensure_electron().await?;
        // let mut cmd = Command::new(electron.exe());

        Ok(())
    }
}