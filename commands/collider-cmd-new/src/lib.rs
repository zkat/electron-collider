use std::path::{Path, PathBuf};

use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    dialoguer::{Input, theme::ColorfulTheme},
    tracing, ColliderCommand,
};

use collider_common::{
    miette::{IntoDiagnostic, Result},
    smol::{fs, prelude::*},
};

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
        let current_dir = std::env::current_dir().into_diagnostic()?;

        let name : String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("What's your project name?: ")
            .with_initial_text("")
            .default("new-collider-project".into())
            .interact_text().into_diagnostic()?;

        match self.template.as_ref() {
            "react" => println!(
                "Making a new React-based Electron app {} at {}",
                &name,
                current_dir.join(&self.path).display(),
            ),
            "typescript" => println!(
                "Making a new Typescript-based Electron app {} at {}",
                &name,
                current_dir.join(&self.path).display(),
            ),
            "vanilla" => println!(
                "Making a new Javascript-based Electron app {} at {}",
                &name,
                current_dir.join(&self.path).display(),
            ),
            template => panic!(
                "Unknown workload: {}, possible workloads are: react, vue, vanilla",
                template
            ),
        }

        self.create_new_directory(&current_dir, &name).await?;
        println!("Created a new Electron project, {}!", name);

        let initialize : String = Input::new()
            .with_prompt("Would you like to install and start the project now? (yes/no)")
            .with_initial_text("")
            .interact_text().into_diagnostic()?;
        if (initialize == "yes") {
            self.init();
        }

        Ok(())
    }
}

impl NewCmd {
    async fn create_new_directory(&self, dir: &PathBuf, name: &String) -> Result<()> {
        let project_path = dir.join(&self.path).join(name);
        fs::create_dir(&project_path).await.into_diagnostic()?;

        // TODO: use walkdir here and preload some of the files with author
        // project names and license data, etc
        let template_path = Path::new("./commands/collider-cmd-new/templates/quick-start");
        let mut entries = fs::read_dir(template_path).await.into_diagnostic()?;
        while let Some(res) = entries.next().await {
            let entry = res.into_diagnostic()?;
            let path = entry.path();
            match path.file_name() {
                Some(filename) => {
                    let dest_path = &project_path.join(filename);
                    fs::copy(&path, &dest_path).await.into_diagnostic()?;
                }
                None => {
                    println!("failed: {:?}", path);
                }
            }
        }
        Ok(())
    }

    async fn init(&self) -> Result<()> {
        // spawn npm i & npm run start here
        // can we use the collider-electron crate here to
        // make the .exe and point at the right output?
        Ok(())
    }
}