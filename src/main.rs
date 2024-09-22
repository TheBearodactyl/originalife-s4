use anyhow::{Context, Result};
use octocrab::Octocrab;
use reqwest;
use reqwest::Url;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::str::FromStr;
use tokio;
use zip::ZipArchive;

fn remove_dir_contents<P: AsRef<std::path::Path>>(path: P) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let octocrab = Octocrab::builder()
        .build()
        .context("Failed to build Octocrab client")?;
    let repo = octocrab.repos("thebearodactyl", "originalife-s4");
    let latest_release = repo
        .releases()
        .get_latest()
        .await
        .context("Failed to fetch latest release")?;

    if let Some(asset) = latest_release
        .assets
        .iter()
        .find(|a| a.name == "updated-pack.zip")
    {
        let client = reqwest::Client::new();
        let response = client
            .get(Url::from_str(asset.browser_download_url.as_str()).expect("fuck."))
            .send()
            .await
            .context("Failed to download asset")?;
        let content = response
            .bytes()
            .await
            .context("Failed to read asset content")?;

        let temp_file = PathBuf::from("updated-pack.zip");
        fs::write(&temp_file, content).context("Failed to write temporary file")?;

        println!("Which launcher do you use?");
        println!("1. Modrinth");
        println!("2. CurseForge");
        println!("3. Prism");
        print!("Enter your choice (1-3): ");
        io::stdout().flush().context("Failed to flush stdout")?;

        let mut choice = String::new();
        io::stdin()
            .read_line(&mut choice)
            .context("Failed to read user input")?;

        let profile_dir = match choice.trim() {
            "1" => env::var("APPDATA").context("Failed to get APPDATA")? + r"\ModrinthApp\profiles",
            "2" => {
                env::var("HOMEDRIVE").context("Failed to get HOMEDRIVE")?
                    + &env::var("HOMEPATH").context("Failed to get HOMEPATH")?
                    + r"\curseforge\minecraft\Instances"
            }
            "3" => {
                env::var("APPDATA").context("Failed to get APPDATA")? + r"\PrismLauncher\instances"
            }
            _ => anyhow::bail!("Invalid choice"),
        };

        let target_dir = PathBuf::from(&profile_dir).join("Originalife Season 4");
        if target_dir.exists() {
            remove_dir_contents(&target_dir).context("Failed to clean target directory")?;
        } else {
            fs::create_dir_all(&target_dir).context("Failed to create target directory")?;
        }

        let file = fs::File::open(&temp_file).context("Failed to open temporary file")?;
        let mut archive = ZipArchive::new(file).context("Failed to create ZIP archive")?;
        archive
            .extract(&target_dir)
            .context("Failed to extract ZIP archive")?;

        let minecraft_dir = target_dir.join(".minecraft");
        if !minecraft_dir.exists() {
            fs::create_dir(&minecraft_dir).context("Failed to create .minecraft directory")?;
        }

        fs::remove_file(temp_file).context("Failed to remove temporary file")?;

        println!("Update completed successfully!");
    } else {
        println!("No new release found or 'updated-pack.zip' not available.");
    }

    Ok(())
}
