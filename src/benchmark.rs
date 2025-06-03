use anyhow::{Context, Result};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub file_size_mb: u64,
    pub reflink_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub filesystem: String,
    pub file_size_mb: u64,
    pub reflink_count: u32,
    pub total_duration: Duration,
    pub avg_reflink_time: Duration,
    pub concurrent_results: ConcurrentBenchmarkResult,
    pub throughput_mb_per_sec: f64,
    pub operations_per_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrentBenchmarkResult {
    pub thread_counts: Vec<u32>,
    pub durations: Vec<Duration>,
    pub operations_per_sec: Vec<f64>,
    pub contention_ratios: Vec<f64>,
}

pub struct BenchmarkRunner {
    mount_point: PathBuf,
    config: BenchmarkConfig,
}

impl BenchmarkRunner {
    pub fn new(mount_point: &Path, config: BenchmarkConfig) -> Self {
        Self {
            mount_point: mount_point.to_path_buf(),
            config,
        }
    }

    pub async fn run_benchmark(&self) -> Result<BenchmarkResult> {
        println!("📁 Creating source file...");
        let source_file = self.create_source_file().await?;

        println!("🔗 Running sequential reflink + write benchmark...");
        let sequential_result = self.run_sequential_benchmark(&source_file).await?;

        println!("⚡ Running concurrent reflink + write benchmarks...");
        let concurrent_result = self.run_concurrent_benchmarks(&source_file).await?;

        let total_data_mb = self.config.file_size_mb * self.config.reflink_count as u64;
        let throughput_mb_per_sec = total_data_mb as f64 / sequential_result.as_secs_f64();
        let operations_per_sec = self.config.reflink_count as f64 / sequential_result.as_secs_f64();

        Ok(BenchmarkResult {
            filesystem: "unknown".to_string(), // Will be set by caller
            file_size_mb: self.config.file_size_mb,
            reflink_count: self.config.reflink_count,
            total_duration: sequential_result,
            avg_reflink_time: sequential_result / self.config.reflink_count,
            concurrent_results: concurrent_result,
            throughput_mb_per_sec,
            operations_per_sec,
        })
    }

    async fn create_source_file(&self) -> Result<PathBuf> {
        let source_path = self.mount_point.join("source_file.dat");
        let mut file = File::create(&source_path)
            .await
            .context("Failed to create source file")?;

        let size_bytes = self.config.file_size_mb * 1024 * 1024;
        let chunk_size = 1024 * 1024; // 1MB chunks
        let mut rng = rand::thread_rng();

        for _ in 0..(size_bytes / chunk_size) {
            let mut chunk = vec![0u8; chunk_size as usize];
            rng.fill(&mut chunk[..]);
            file.write_all(&chunk)
                .await
                .context("Failed to write to source file")?;
        }

        file.sync_all()
            .await
            .context("Failed to sync source file")?;

        Ok(source_path)
    }

    async fn run_sequential_benchmark(&self, source_file: &Path) -> Result<Duration> {
        let start = Instant::now();

        for i in 0..self.config.reflink_count {
            let target_path = self.mount_point.join(format!("reflink_{}.dat", i));
            self.create_reflink(source_file, &target_path)
                .await
                .context(format!("Failed to create reflink {}", i))?;
        }

        Ok(start.elapsed())
    }

    async fn run_concurrent_benchmarks(
        &self,
        source_file: &Path,
    ) -> Result<ConcurrentBenchmarkResult> {
        let thread_counts = vec![1, 2, 4, 8, 16, 32, 64, 128];
        let mut durations = Vec::new();
        let mut operations_per_sec = Vec::new();
        let mut contention_ratios = Vec::new();

        let baseline_duration = self
            .run_concurrent_with_threads(source_file, 1, "concurrent")
            .await?;
        let baseline_ops_per_sec =
            self.config.reflink_count as f64 / baseline_duration.as_secs_f64();

        for &thread_count in &thread_counts {
            println!("  Testing with {} concurrent threads...", thread_count);

            let duration = self
                .run_concurrent_with_threads(
                    source_file,
                    thread_count,
                    &format!("concurrent_{}", thread_count),
                )
                .await?;

            let ops_per_sec = self.config.reflink_count as f64 / duration.as_secs_f64();
            let contention_ratio = baseline_ops_per_sec / ops_per_sec;

            durations.push(duration);
            operations_per_sec.push(ops_per_sec);
            contention_ratios.push(contention_ratio);

            println!(
                "    Duration: {:?}, Ops/sec: {:.2}, Contention ratio: {:.2}x",
                duration, ops_per_sec, contention_ratio
            );
        }

        Ok(ConcurrentBenchmarkResult {
            thread_counts,
            durations,
            operations_per_sec,
            contention_ratios,
        })
    }

    async fn run_concurrent_with_threads(
        &self,
        source_file: &Path,
        thread_count: u32,
        prefix: &str,
    ) -> Result<Duration> {
        let semaphore = Arc::new(Semaphore::new(thread_count as usize));
        let mut join_set = JoinSet::new();
        let operations_per_thread = (self.config.reflink_count + thread_count - 1) / thread_count;

        let start = Instant::now();

        for thread_id in 0..thread_count {
            let semaphore = Arc::clone(&semaphore);
            let source_file = source_file.to_path_buf();
            let mount_point = self.mount_point.clone();
            let start_idx = thread_id * operations_per_thread;
            let end_idx = ((thread_id + 1) * operations_per_thread).min(self.config.reflink_count);
            let prefix = prefix.to_string();

            if start_idx >= self.config.reflink_count {
                break;
            }

            join_set.spawn(async move {
                let _permit = semaphore.acquire().await.unwrap();

                for i in start_idx..end_idx {
                    let target_path = mount_point.join(format!("{}_{}.dat", prefix, i));
                    let source_file_clone = source_file.clone();

                    if let Err(e) = tokio::task::spawn_blocking(move || {
                        Self::create_reflink_and_write_blocking(&source_file_clone, &target_path)
                    })
                    .await
                    .context("Task panicked")?
                    {
                        eprintln!("Failed to create reflink and write {}: {}", i, e);
                        return Err(e);
                    }
                }

                Ok(())
            });
        }

        // Wait for all tasks to complete
        while let Some(result) = join_set.join_next().await {
            result.context("Task panicked")??;
        }

        Ok(start.elapsed())
    }

    async fn create_reflink(&self, source: &Path, target: &Path) -> Result<()> {
        let source = source.to_path_buf();
        let target = target.to_path_buf();
        tokio::task::spawn_blocking(move || {
            Self::create_reflink_and_write_blocking(&source, &target)
        })
        .await
        .context("Task panicked")?
    }

    fn create_reflink_and_write_blocking(source: &Path, target: &Path) -> Result<()> {
        use std::io::{Seek, SeekFrom, Write};
        use std::os::unix::io::AsRawFd;

        // Open source file
        let source_file = std::fs::File::open(source).context("Failed to open source file")?;

        // Create target file
        let mut target_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .truncate(true)
            .open(target)
            .context("Failed to create target file")?;

        // Use FICLONE ioctl for reflink operation
        let result = unsafe {
            libc::ioctl(
                target_file.as_raw_fd(),
                0x40049409, // FICLONE
                source_file.as_raw_fd(),
            )
        };

        if result != 0 {
            let errno = std::io::Error::last_os_error();
            anyhow::bail!(
                "Reflink operation failed: {}. Filesystem may not support reflinks.",
                errno
            );
        }

        // Write some data to trigger copy-on-write
        // Write to the beginning of the file to ensure CoW is triggered
        target_file
            .seek(SeekFrom::Start(0))
            .context("Failed to seek to beginning of target file")?;

        // Write a small amount of data (4KB) to trigger CoW without significantly affecting timing
        let write_data = vec![0xAA; 4096];
        target_file
            .write_all(&write_data)
            .context("Failed to write to target file")?;

        Ok(())
    }

    #[allow(dead_code)]
    fn create_reflink_blocking(source: &Path, target: &Path) -> Result<()> {
        use std::os::unix::io::AsRawFd;

        // Open source file
        let source_file = std::fs::File::open(source).context("Failed to open source file")?;

        // Create target file
        let target_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(target)
            .context("Failed to create target file")?;

        // Use FICLONE ioctl for reflink operation
        let result = unsafe {
            libc::ioctl(
                target_file.as_raw_fd(),
                0x40049409, // FICLONE
                source_file.as_raw_fd(),
            )
        };

        if result != 0 {
            let errno = std::io::Error::last_os_error();
            anyhow::bail!(
                "Reflink operation failed: {}. Filesystem may not support reflinks.",
                errno
            );
        }

        Ok(())
    }
}
