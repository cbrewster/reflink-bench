use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tabled::{Table, Tabled};

use crate::benchmark::BenchmarkResult;
use crate::filesystem::FilesystemType;

#[derive(Debug, Serialize, Deserialize)]
pub struct ComparisonResults {
    pub results: Vec<(String, BenchmarkResult)>,
    pub timestamp: String,
}

pub struct ResultsReporter {
    results: Vec<(FilesystemType, BenchmarkResult)>,
}

#[derive(Tabled)]
struct ComparisonRow {
    #[tabled(rename = "Filesystem")]
    filesystem: String,
    #[tabled(rename = "File Size (MB)")]
    file_size: u64,
    #[tabled(rename = "Reflinks")]
    reflink_count: u32,
    #[tabled(rename = "Total Time")]
    total_time: String,
    #[tabled(rename = "Avg per Reflink")]
    avg_time: String,
    #[tabled(rename = "Throughput (MB/s)")]
    throughput: String,
    #[tabled(rename = "Ops/sec")]
    ops_per_sec: String,
}

#[derive(Tabled)]
struct ConcurrencyRow {
    #[tabled(rename = "Threads")]
    threads: u32,
    #[tabled(rename = "XFS Ops/sec")]
    xfs_ops: String,
    #[tabled(rename = "btrfs Ops/sec")]
    btrfs_ops: String,
    #[tabled(rename = "XFS Contention")]
    xfs_contention: String,
    #[tabled(rename = "btrfs Contention")]
    btrfs_contention: String,
    #[tabled(rename = "Winner")]
    winner: String,
}

impl ResultsReporter {
    pub fn new(results: Vec<(FilesystemType, BenchmarkResult)>) -> Self {
        Self { results }
    }

    pub fn print_comparison(&self) {
        println!("\n📊 FILESYSTEM COMPARISON RESULTS");
        println!("================================");

        // Main comparison table
        let comparison_rows: Vec<ComparisonRow> = self
            .results
            .iter()
            .map(|(fs_type, result)| ComparisonRow {
                filesystem: format!("{}", fs_type),
                file_size: result.file_size_mb,
                reflink_count: result.reflink_count,
                total_time: format_duration(result.total_duration),
                avg_time: format_duration(result.avg_reflink_time),
                throughput: format!("{:.2}", result.throughput_mb_per_sec),
                ops_per_sec: format!("{:.2}", result.operations_per_sec),
            })
            .collect();

        let table = Table::new(comparison_rows);
        println!("{}", table);

        // Concurrency analysis
        if self.results.len() == 2 {
            self.print_concurrency_analysis();
        }

        // Performance summary
        self.print_performance_summary();
    }

    fn print_concurrency_analysis(&self) {
        println!("\n⚡ CONCURRENCY PERFORMANCE ANALYSIS");
        println!("==================================");

        let xfs_result = self
            .results
            .iter()
            .find(|(fs, _)| matches!(fs, FilesystemType::Xfs));
        let btrfs_result = self
            .results
            .iter()
            .find(|(fs, _)| matches!(fs, FilesystemType::Btrfs));

        if let (Some((_, xfs)), Some((_, btrfs))) = (xfs_result, btrfs_result) {
            let thread_counts = &xfs.concurrent_results.thread_counts;
            let concurrency_rows: Vec<ConcurrencyRow> = thread_counts
                .iter()
                .enumerate()
                .map(|(i, &threads)| {
                    let xfs_ops = xfs.concurrent_results.operations_per_sec[i];
                    let btrfs_ops = btrfs.concurrent_results.operations_per_sec[i];
                    let xfs_contention = xfs.concurrent_results.contention_ratios[i];
                    let btrfs_contention = btrfs.concurrent_results.contention_ratios[i];

                    let winner = if xfs_ops > btrfs_ops { "XFS" } else { "btrfs" };
                    let advantage = if xfs_ops > btrfs_ops {
                        xfs_ops / btrfs_ops
                    } else {
                        btrfs_ops / xfs_ops
                    };

                    ConcurrencyRow {
                        threads,
                        xfs_ops: format!("{:.1}", xfs_ops),
                        btrfs_ops: format!("{:.1}", btrfs_ops),
                        xfs_contention: format!("{:.2}x", xfs_contention),
                        btrfs_contention: format!("{:.2}x", btrfs_contention),
                        winner: format!("{} ({:.1}x faster)", winner, advantage),
                    }
                })
                .collect();

            let table = Table::new(concurrency_rows);
            println!("{}", table);

            // Contention analysis
            println!("\n🔥 CONTENTION ANALYSIS");
            println!("=====================");

            let max_xfs_contention = xfs
                .concurrent_results
                .contention_ratios
                .iter()
                .fold(0.0f64, |a, &b| a.max(b));
            let max_btrfs_contention = btrfs
                .concurrent_results
                .contention_ratios
                .iter()
                .fold(0.0f64, |a, &b| a.max(b));

            println!("Maximum contention ratios:");
            println!(
                "  XFS: {:.2}x slower at high concurrency",
                max_xfs_contention
            );
            println!(
                "  btrfs: {:.2}x slower at high concurrency",
                max_btrfs_contention
            );

            if max_xfs_contention < max_btrfs_contention {
                println!("  🏆 XFS shows better concurrency scaling");
            } else {
                println!("  🏆 btrfs shows better concurrency scaling");
            }
        }
    }

    fn print_performance_summary(&self) {
        println!("\n🏁 PERFORMANCE SUMMARY");
        println!("=====================");

        if self.results.len() == 2 {
            let xfs_result = self
                .results
                .iter()
                .find(|(fs, _)| matches!(fs, FilesystemType::Xfs));
            let btrfs_result = self
                .results
                .iter()
                .find(|(fs, _)| matches!(fs, FilesystemType::Btrfs));

            if let (Some((_, xfs)), Some((_, btrfs))) = (xfs_result, btrfs_result) {
                let xfs_faster_sequential = xfs.operations_per_sec > btrfs.operations_per_sec;
                let sequential_ratio = if xfs_faster_sequential {
                    xfs.operations_per_sec / btrfs.operations_per_sec
                } else {
                    btrfs.operations_per_sec / xfs.operations_per_sec
                };

                println!("Sequential Performance:");
                if xfs_faster_sequential {
                    println!("  🏆 XFS is {:.1}x faster than btrfs", sequential_ratio);
                } else {
                    println!("  🏆 btrfs is {:.1}x faster than XFS", sequential_ratio);
                }

                // Find best concurrent performance for each
                let xfs_best_concurrent = xfs
                    .concurrent_results
                    .operations_per_sec
                    .iter()
                    .fold(0.0f64, |a, &b| a.max(b));
                let btrfs_best_concurrent = btrfs
                    .concurrent_results
                    .operations_per_sec
                    .iter()
                    .fold(0.0f64, |a, &b| a.max(b));

                let concurrent_ratio = if xfs_best_concurrent > btrfs_best_concurrent {
                    xfs_best_concurrent / btrfs_best_concurrent
                } else {
                    btrfs_best_concurrent / xfs_best_concurrent
                };

                println!("Best Concurrent Performance:");
                if xfs_best_concurrent > btrfs_best_concurrent {
                    println!(
                        "  🏆 XFS is {:.1}x faster than btrfs at optimal concurrency",
                        concurrent_ratio
                    );
                } else {
                    println!(
                        "  🏆 btrfs is {:.1}x faster than XFS at optimal concurrency",
                        concurrent_ratio
                    );
                }
            }
        }

        println!("\nRecommendations:");
        println!(
            "  📈 Use these results to choose the optimal filesystem for your reflink workload"
        );
        println!("  ⚙️  Consider the concurrency patterns of your application");
        println!("  🔧 Test with your specific file sizes and access patterns");
    }

    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let comparison_results = ComparisonResults {
            results: self
                .results
                .iter()
                .map(|(fs_type, result)| (format!("{}", fs_type), (*result).clone()))
                .collect::<Vec<_>>(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        let json = serde_json::to_string_pretty(&comparison_results)
            .context("Failed to serialize results")?;

        std::fs::write(path, json).context("Failed to write results file")?;

        Ok(())
    }
}

fn format_duration(duration: Duration) -> String {
    let total_ms = duration.as_millis();
    if total_ms < 1000 {
        format!("{}ms", total_ms)
    } else if total_ms < 60000 {
        format!("{:.2}s", duration.as_secs_f64())
    } else {
        let minutes = total_ms / 60000;
        let seconds = (total_ms % 60000) as f64 / 1000.0;
        format!("{}m{:.1}s", minutes, seconds)
    }
}
