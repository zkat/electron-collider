use std::path::PathBuf;

use collider_command::{
    async_trait::async_trait,
    clap::{self, Clap},
    collider_config::{self, ColliderConfigLayer},
    tracing, ColliderCommand,
};
use collider_common::{
    miette::{IntoDiagnostic, Result},
    smol::{fs, io::AsyncWriteExt},
};

#[derive(Debug, Clap, ColliderConfigLayer)]
pub struct NewCmd {
    #[clap(about = "Path to create new Electron application in.")]
    path: PathBuf,
    #[clap(
        long,
        short = 'n',
        default_value = "new-collider-project",
        about = "Name of your new Electron Collider project."
    )]
    name: String,
    #[clap(
        long,
        short = 't',
        default_value = "react",
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
        self.create_new_directory(&current_dir).await?;

        match self.template.as_ref() {
            "react" => println!(
                "Making a new React-based Electron app at {}",
                current_dir.join(self.path).display(),
            ),
            "vue" => println!(
                "Making a new Vue-based Electron app at {}",
                current_dir.join(self.path).display(),
            ),
            "vanilla" => println!(
                "Making a new VanillaJS-based Electron app at {}",
                current_dir.join(self.path).display(),
            ),
            template => panic!(
                "Unknown workload: {}, possible workloads are: react, vue, vanilla",
                template
            ),
        }
        Ok(())
    }
}

impl NewCmd {
    async fn create_new_directory(&self, dir: &PathBuf) -> Result<()> {
        let project_path = dir.join(&self.path).join(&self.name);
        fs::create_dir(&project_path).await.into_diagnostic()?;

        let mut file = fs::File::create(&project_path.join("README.md")).await.into_diagnostic()?;
        file.write_all(b"Hello, world!").await.into_diagnostic()?;

        let mut package_json = fs::File::create(&project_path.join("package.json")).await.into_diagnostic()?;
        // let mut template_json = File::open((&self.template).join("package.json"))?;
        // let mut contents = String::new();
        // package_json.read_to_string(&mut contents)?;
        Ok(())
    }
}