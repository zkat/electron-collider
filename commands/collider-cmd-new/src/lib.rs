use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    dialoguer::{Input, theme::ColorfulTheme},
    owo_colors::{OwoColorize},
    tracing, ColliderCommand,
};

use collider_common::{
    miette::{self, Context, IntoDiagnostic, Result},
    smol::{fs, process::Command},
};

#[derive(Debug, Clap, ColliderConfigLayer)]
pub struct NewCmd {
    #[clap(about = "Path to create new Electron application in.")]
    path: PathBuf,
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
        println!("Setting up your project...");
        let name : String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Project Name?: ")
            .with_initial_text("")
            .default("new-collider-project".into())
            .interact_text().into_diagnostic()?;
        let proj_path = &self.path.join(&name);

        self.create_new_dir(&proj_path).await?;
        println!(
            "✔ Created Project Directory for {} at {}",
            name.green(),
            proj_path.display().green(),
        );

        self.init_git(&proj_path).await?;
        self.init_npm(&proj_path).await?;

        println!("✨ {} is Ready for Development ✨", &name.green());
        println!("Run \"npm run start\" to see it in action.");

        Ok(())
    }
}

impl NewCmd {
    async fn create_new_dir(&self, dir: &PathBuf) -> Result<()> {
        fs::create_dir(&dir).await.into_diagnostic()?;
        let template_path = Path::new("./commands/collider-cmd-new/templates/quick-start");
        for entry in WalkDir::new(template_path)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
        .filter_map(|e| e.ok()) {
            if entry.metadata().unwrap().is_dir() {
                let options = fs_extra::dir::CopyOptions::new();
                fs_extra::dir::copy(entry.path(), &dir, &options);
            } else if entry.metadata().unwrap().is_file() {
                let path = entry.path();
                match path.file_name() {
                    Some(filename) => {
                        let dest_path = &dir.join(filename);
                        fs::copy(&path, &dest_path).await.into_diagnostic()?;
                    }
                    None => {
                        miette::bail!("Project directory creation failed.");
                    }
                }
            }
        }
        Ok(())
    }

    async fn init_npm(&self, proj_dir: &PathBuf) -> Result<()> {
        let npm_path = which::which("npm").into_diagnostic().context(
            "Failed to find npm command while creating project. NPM/npx are required by Collider.",
        )?;
        
        println!("✔ Installing NPM Dependencies");
        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.arg("/c");
            cmd.arg(npm_path);
            cmd
        } else {
            Command::new(npm_path)
        };

        let status = cmd
            .arg("install")
            .arg("--silent")
            .current_dir(proj_dir)
            .status()
            .await
            .into_diagnostic()
            .context("Failed to spawn NPM itself.")?;

        if !status.success() {
            miette::bail!("Could not initialize project");
        }
        Ok(())
    }

    async fn init_git(&self, proj_dir: &PathBuf) -> Result<()> {
        println!("✔ Initializing Git");
        let git_path = which::which("git").into_diagnostic().context(
            "Failed to find git while creating project. Git is required by Collider."
        )?;

        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.arg("/c");
            cmd.arg(git_path);
            cmd
        } else {
            Command::new(git_path)
        };

        let status = cmd
            .arg("init")
            .arg("--quiet")
            .current_dir(proj_dir)
            .status()
            .await
            .into_diagnostic()
            .context("Failed to initialize git itself.")?;

        if !status.success() {
            miette::bail!("Could not initialize git.");
        }
        Ok(())
    }
}