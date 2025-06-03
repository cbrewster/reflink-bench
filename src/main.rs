use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod benchmark;
mod filesystem;
mod results;

use benchmark::{BenchmarkConfig, BenchmarkRunner};
use filesystem::{FilesystemManager, FilesystemType};
use results::ResultsReporter;

#[derive(Parser)]
#[command(name = "reflink-bench")]
#[command(about = "Benchmark XFS vs btrfs reflink operations")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run benchmarks comparing XFS and btrfs reflink + write performance
    Run {
        /// Size of test files in MB
        #[arg(long, default_value = "100")]
        file_size_mb: u64,

        /// Number of reflink+write operations to perform per test
        #[arg(long, default_value = "1000")]
        reflink_count: u32,

        /// Size of filesystem images in GB
        #[arg(long, default_value = "2")]
        fs_size_gb: u64,

        /// Output results to JSON file
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Clean up any leftover filesystem images and mounts
    Cleanup,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            file_size_mb,
            reflink_count,
            fs_size_gb,
            output,
        } => {
            println!("🚀 Starting reflink + write benchmark suite");
            println!(
                "File size: {}MB, Reflink+write count: {}, FS size: {}GB",
                file_size_mb, reflink_count, fs_size_gb
            );

            let config = BenchmarkConfig {
                file_size_mb,
                reflink_count,
            };

            let mut results = Vec::new();

            for fs_type in [FilesystemType::Xfs, FilesystemType::Btrfs] {
                println!("\n📊 Testing {} filesystem...", fs_type);

                let mut fs_manager = FilesystemManager::new(fs_type, fs_size_gb)?;
                fs_manager.setup().await?;

                let runner = BenchmarkRunner::new(fs_manager.mount_point(), config.clone());
                let mut result = runner.run_benchmark().await?;
                result.filesystem = format!("{}", fs_type);

                results.push((fs_type, result));

                fs_manager.cleanup().await?;
            }

            let reporter = ResultsReporter::new(results);
            reporter.print_comparison();

            if let Some(output_path) = output {
                reporter.save_to_file(&output_path)?;
                println!("\n💾 Results saved to {}", output_path.display());
            }
        }
        Commands::Cleanup => {
            println!("🧹 Cleaning up filesystem artifacts...");
            FilesystemManager::cleanup_all().await?;
            println!("✅ Cleanup completed");
        }
    }

    Ok(())
}
