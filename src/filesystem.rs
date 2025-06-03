use anyhow::{Context, Result};
use nix::mount::{mount, umount, MsFlags};
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;

#[derive(Debug, Clone, Copy)]
pub enum FilesystemType {
    Xfs,
    Btrfs,
}

impl Display for FilesystemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilesystemType::Xfs => write!(f, "XFS"),
            FilesystemType::Btrfs => write!(f, "btrfs"),
        }
    }
}

pub struct FilesystemManager {
    fs_type: FilesystemType,
    size_gb: u64,
    image_path: PathBuf,
    mount_point: PathBuf,
    loop_device: Option<String>,
}

impl FilesystemManager {
    pub fn new(fs_type: FilesystemType, size_gb: u64) -> Result<Self> {
        let image_path = PathBuf::from(format!(
            "/tmp/reflink-bench-{}.img",
            format!("{:?}", fs_type).to_lowercase()
        ));
        let mount_point = PathBuf::from(format!(
            "/tmp/reflink-bench-{}",
            format!("{:?}", fs_type).to_lowercase()
        ));

        Ok(Self {
            fs_type,
            size_gb,
            image_path,
            mount_point,
            loop_device: None,
        })
    }

    pub async fn setup(&mut self) -> Result<()> {
        self.cleanup_existing().await?;
        self.create_image().await?;
        self.setup_loop_device().await?;
        self.format_filesystem().await?;
        self.create_mount_point().await?;
        self.mount_filesystem().await?;
        Ok(())
    }

    pub async fn cleanup(&self) -> Result<()> {
        if self.mount_point.exists() {
            let _ = self.unmount_filesystem().await;
        }

        if let Some(loop_dev) = &self.loop_device {
            let _ = self.detach_loop_device(loop_dev).await;
        }

        if self.image_path.exists() {
            fs::remove_file(&self.image_path)
                .await
                .context("Failed to remove filesystem image")?;
        }

        if self.mount_point.exists() {
            fs::remove_dir(&self.mount_point)
                .await
                .context("Failed to remove mount point")?;
        }

        Ok(())
    }

    pub async fn cleanup_all() -> Result<()> {
        for fs_type in [FilesystemType::Xfs, FilesystemType::Btrfs] {
            let manager = Self::new(fs_type, 1)?; // Size doesn't matter for cleanup
            let _ = manager.cleanup().await; // Ignore errors during cleanup
        }
        Ok(())
    }

    pub fn mount_point(&self) -> &Path {
        &self.mount_point
    }

    async fn cleanup_existing(&self) -> Result<()> {
        if self.mount_point.exists() {
            let _ = umount(&self.mount_point);
        }

        if self.image_path.exists() {
            fs::remove_file(&self.image_path).await.ok();
        }

        Ok(())
    }

    async fn create_image(&self) -> Result<()> {
        let size_mb = self.size_gb * 1024;

        let output = Command::new("dd")
            .args([
                "if=/dev/zero",
                &format!("of={}", self.image_path.display()),
                "bs=1M",
                &format!("count={}", size_mb),
            ])
            .output()
            .context("Failed to create filesystem image")?;

        if !output.status.success() {
            anyhow::bail!("dd failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(())
    }

    async fn setup_loop_device(&mut self) -> Result<()> {
        let output = Command::new("losetup")
            .args(["-f", "--show", &self.image_path.to_string_lossy()])
            .output()
            .context("Failed to setup loop device")?;

        if !output.status.success() {
            anyhow::bail!(
                "losetup failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let loop_device = String::from_utf8(output.stdout)
            .context("Invalid UTF-8 in losetup output")?
            .trim()
            .to_string();

        self.loop_device = Some(loop_device);
        Ok(())
    }

    async fn format_filesystem(&self) -> Result<()> {
        let loop_device = self
            .loop_device
            .as_ref()
            .context("Loop device not set up")?;

        let (cmd, args): (&str, Vec<&str>) = match self.fs_type {
            FilesystemType::Xfs => ("mkfs.xfs", vec!["-f", loop_device]),
            FilesystemType::Btrfs => ("mkfs.btrfs", vec!["-f", loop_device]),
        };

        let output = Command::new(cmd)
            .args(args)
            .output()
            .context(format!("Failed to format {} filesystem", self.fs_type))?;

        if !output.status.success() {
            anyhow::bail!(
                "{} failed: {}",
                cmd,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }

    async fn create_mount_point(&self) -> Result<()> {
        if !self.mount_point.exists() {
            fs::create_dir_all(&self.mount_point)
                .await
                .context("Failed to create mount point")?;
        }
        Ok(())
    }

    async fn mount_filesystem(&self) -> Result<()> {
        let loop_device = self
            .loop_device
            .as_ref()
            .context("Loop device not set up")?;

        let fs_type_str = match self.fs_type {
            FilesystemType::Xfs => "xfs",
            FilesystemType::Btrfs => "btrfs",
        };

        mount(
            Some(loop_device.as_str()),
            &self.mount_point,
            Some(fs_type_str),
            MsFlags::empty(),
            None::<&str>,
        )
        .context("Failed to mount filesystem")?;

        // Set permissions for non-root access
        Command::new("chmod")
            .args(["777", &self.mount_point.to_string_lossy()])
            .output()
            .context("Failed to set mount point permissions")?;

        Ok(())
    }

    async fn unmount_filesystem(&self) -> Result<()> {
        umount(&self.mount_point).context("Failed to unmount filesystem")?;
        Ok(())
    }

    async fn detach_loop_device(&self, loop_device: &str) -> Result<()> {
        let output = Command::new("losetup")
            .args(["-d", loop_device])
            .output()
            .context("Failed to detach loop device")?;

        if !output.status.success() {
            anyhow::bail!(
                "losetup -d failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }
}
