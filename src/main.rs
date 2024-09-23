use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use octocrab::Octocrab;
use reqwest;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use tokio;
use zip::ZipArchive;

fn remove_dir_contents<P: AsRef<Path>>(path: P) -> Result<()> {
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

fn get_cache_dir() -> PathBuf {
    let mut cache_dir = env::temp_dir();
    cache_dir.push("originalife_s4_cache");
    fs::create_dir_all(&cache_dir).expect("Failed to create cache directory");
    cache_dir
}

fn get_cached_file_path(artifact_name: &str, sha256: &str) -> PathBuf {
    let mut cache_file = get_cache_dir();
    cache_file.push(format!("{}-{}", sha256, artifact_name));
    cache_file
}

async fn download_file(url: &str, total_size: u64, sha256: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::new();
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})").expect("fuck.")
        .progress_chars("#>-"));

    let mut response = client
        .get(url)
        .send()
        .await
        .context("Failed to send request")?;
    let mut content = Vec::with_capacity(total_size as usize);

    while let Some(chunk) = response.chunk().await.context("Failed to read chunk")? {
        content.extend_from_slice(&chunk);
        pb.inc(chunk.len() as u64);
    }

    pb.finish_with_message("Download completed");

    // Verify SHA256
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let result = hasher.finalize();
    let downloaded_sha256 = format!("{:x}", result);

    if downloaded_sha256 != sha256 {
        anyhow::bail!("SHA256 mismatch for downloaded file");
    }

    Ok(content)
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
    let choice = choice.trim();

    let artifact_name = match choice {
        "1" => "updated-pack-modrinth.zip",
        "2" => "updated-pack-curseforge.zip",
        _ => "updated-pack-prism.zip",
    };

    if let Some(asset) = latest_release
        .assets
        .iter()
        .find(|a| a.name == artifact_name)
    {
        let url = asset.browser_download_url.as_str();
        let total_size = asset.size;
        let sha256 = &asset.name[..64];

        let cache_file_path = get_cached_file_path(artifact_name, sha256);
        let content = if cache_file_path.exists() {
            println!("Using cached file");
            let mut file =
                fs::File::open(&cache_file_path).context("Failed to open cached file")?;
            let mut content = Vec::new();
            file.read_to_end(&mut content)
                .context("Failed to read cached file")?;
            content
        } else {
            let content = download_file(url, total_size as u64, sha256).await?;
            fs::write(&cache_file_path, &content).context("Failed to write cache file")?;
            content
        };

        let temp_file = PathBuf::from(artifact_name);
        fs::write(&temp_file, content).context("Failed to write temporary file")?;

        let profile_dir = match choice {
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

        fs::remove_file(temp_file).context("Failed to remove temporary file")?;

        println!("Update completed successfully!");
    } else {
        println!("No new release found or '{}' not available.", artifact_name);
    }

    Ok(())
}
