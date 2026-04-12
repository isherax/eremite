use anyhow::Result;
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};

use eremite_models::ModelManager;

#[derive(Parser)]
#[command(name = "eremite-models", about = "Manage local GGUF models")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Download a GGUF model from Hugging Face Hub
    Download {
        /// Hugging Face repo ID (e.g. bartowski/Llama-3.2-1B-Instruct-GGUF)
        repo_id: String,
        /// Filename to download (e.g. Llama-3.2-1B-Instruct-Q4_K_M.gguf)
        filename: String,
    },
    /// List all downloaded models
    List,
    /// Show details of a downloaded model
    Info {
        /// Hugging Face repo ID
        repo_id: String,
        /// Filename
        filename: String,
    },
    /// Remove a downloaded model
    Remove {
        /// Hugging Face repo ID
        repo_id: String,
        /// Filename
        filename: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Download { repo_id, filename } => {
            let mut manager = ModelManager::default_path()?;

            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::with_template(
                    "{spinner:.green} [{elapsed_precise}] {bytes}/{total_bytes} ({bytes_per_sec})",
                )
                .unwrap(),
            );

            let entry = manager
                .download_with_progress(&repo_id, &filename, None, |downloaded, total| {
                    if let Some(total) = total {
                        pb.set_length(total);
                    }
                    pb.set_position(downloaded);
                })
                .await?;

            pb.finish_and_clear();
            println!("Downloaded: {}/{}", entry.repo_id, entry.filename);
            println!("  Size:   {} bytes", entry.size_bytes);
            println!("  SHA256: {}", entry.sha256);
            println!(
                "  Path:   {}",
                manager.model_path(&repo_id, &filename).display()
            );
        }
        Commands::List => {
            let manager = ModelManager::default_path()?;
            let models = manager.list();

            if models.is_empty() {
                println!("No models downloaded.");
                return Ok(());
            }

            println!(
                "{:<45} {:<40} {:>12}",
                "REPO", "FILENAME", "SIZE"
            );
            for entry in models {
                println!(
                    "{:<45} {:<40} {:>12}",
                    entry.repo_id,
                    entry.filename,
                    format_bytes(entry.size_bytes),
                );
            }
        }
        Commands::Info { repo_id, filename } => {
            let manager = ModelManager::default_path()?;

            match manager.get(&repo_id, &filename) {
                Some(entry) => {
                    println!("Repo:     {}", entry.repo_id);
                    println!("Filename: {}", entry.filename);
                    println!("Size:     {} ({})", format_bytes(entry.size_bytes), entry.size_bytes);
                    println!("SHA256:   {}", entry.sha256);
                    println!("Downloaded: {}", entry.downloaded_at);
                    println!(
                        "Path:     {}",
                        manager.model_path(&repo_id, &filename).display()
                    );
                }
                None => {
                    println!("Model {}/{} not found.", repo_id, filename);
                }
            }
        }
        Commands::Remove { repo_id, filename } => {
            let mut manager = ModelManager::default_path()?;
            manager.remove(&repo_id, &filename)?;
            println!("Removed {}/{}", repo_id, filename);
        }
    }

    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
