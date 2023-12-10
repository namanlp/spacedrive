// Adapted from: https://github.com/kimlimjustin/xplorer/blob/f4f3590d06783d64949766cc2975205a3b689a56/src-tauri/src/drives.rs

use std::{
	fmt::Display,
	hash::{Hash, Hasher},
	path::PathBuf,
	sync::OnceLock,
};

use sd_cache::Model;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use specta::Type;
use sysinfo::{DiskExt, System, SystemExt};
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::error;

pub mod watcher;

fn sys_guard() -> &'static Mutex<System> {
	static SYS: OnceLock<Mutex<System>> = OnceLock::new();
	SYS.get_or_init(|| Mutex::new(System::new_all()))
}

#[derive(Serialize, Deserialize, Debug, Clone, Type, Hash, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum DiskType {
	SSD,
	HDD,
	Removable,
}

impl Display for DiskType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			Self::SSD => "SSD",
			Self::HDD => "HDD",
			Self::Removable => "Removable",
		})
	}
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct Volume {
	pub name: String,
	pub mount_points: Vec<PathBuf>,
	#[specta(type = String)]
	#[serde_as(as = "DisplayFromStr")]
	pub total_capacity: u64,
	#[specta(type = String)]
	#[serde_as(as = "DisplayFromStr")]
	pub available_capacity: u64,
	pub disk_type: DiskType,
	pub file_system: Option<String>,
	pub is_root_filesystem: bool,
}

impl Model for Volume {
	fn name() -> &'static str {
		"Volume"
	}
}

impl Hash for Volume {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.name.hash(state);
		self.mount_points.iter().for_each(|mount_point| {
			// Hashing like this to ignore ordering between mount points
			mount_point.hash(state);
		});
		self.disk_type.hash(state);
		self.file_system.hash(state);
	}
}

impl PartialEq for Volume {
	fn eq(&self, other: &Self) -> bool {
		self.name == other.name
			&& self.disk_type == other.disk_type
			&& self.file_system == other.file_system
			// Leaving mount points for last because O(n * m)
			&& self
				.mount_points
				.iter()
				.all(|mount_point| other.mount_points.contains(mount_point))
	}
}

impl Eq for Volume {}

#[derive(Error, Debug)]
pub enum VolumeError {
	#[error("Database error: {0}")]
	DatabaseErr(#[from] prisma_client_rust::QueryError),
	#[error("FromUtf8Error: {0}")]
	FromUtf8Error(#[from] std::string::FromUtf8Error),
}

impl From<VolumeError> for rspc::Error {
	fn from(e: VolumeError) -> Self {
		rspc::Error::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e)
	}
}

#[cfg(target_os = "linux")]
pub async fn get_volumes() -> Vec<Volume> {
	use std::{collections::HashMap, path::Path};

	let mut sys = sys_guard().lock().await;
	sys.refresh_disks_list();

	let mut volumes: Vec<Volume> = Vec::new();
	let mut path_to_volume_index = HashMap::new();
	for disk in sys.disks() {
		let disk_name = disk.name();
		let mount_point = disk.mount_point().to_path_buf();
		let file_system = String::from_utf8(disk.file_system().to_vec())
			.map(|s| s.to_uppercase())
			.ok();
		let total_capacity = disk.total_space();
		let available_capacity = disk.available_space();
		let is_root_filesystem = mount_point.is_absolute() && mount_point.parent().is_none();

		let mut disk_path: PathBuf = PathBuf::from(disk_name);
		if file_system.as_ref().map(|fs| fs == "ZFS").unwrap_or(false) {
			// Use a custom path for ZFS disks to avoid conflicts with normal disks paths
			disk_path = Path::new("zfs://").join(disk_path);
		} else {
			// Ignore non-devices disks (overlay, fuse, tmpfs, etc.)
			if !disk_path.starts_with("/dev") {
				continue;
			}

			// Ensure disk has a valid device path
			let real_path = match tokio::fs::canonicalize(disk_name).await {
				Err(real_path) => {
					error!(
						"Failed to canonicalize disk path {}: {:#?}",
						disk_name.to_string_lossy(),
						real_path
					);
					continue;
				}
				Ok(real_path) => real_path,
			};

			// Check if disk is a symlink to another disk
			if real_path != disk_path {
				// Disk is a symlink to another disk, assign it to the same volume
				path_to_volume_index.insert(
					real_path.into_os_string(),
					path_to_volume_index
						.get(disk_name)
						.cloned()
						.unwrap_or(path_to_volume_index.len()),
				);
			}
		}

		if let Some(volume_index) = path_to_volume_index.get(disk_name) {
			// Disk already has a volume assigned, update it
			let volume: &mut Volume = volumes
				.get_mut(*volume_index)
				.expect("Volume index is present so the Volume must be present too");

			// Update mount point if not already present
			let mount_points = &mut volume.mount_points;
			if mount_point.iter().all(|p| *p != mount_point) {
				mount_points.push(mount_point);
				let mount_points_to_check = mount_points.clone();
				mount_points.retain(|candidate| {
					!mount_points_to_check
						.iter()
						.any(|path| candidate.starts_with(path) && candidate != path)
				});
				if !volume.is_root_filesystem {
					volume.is_root_filesystem = is_root_filesystem;
				}
			}

			// Update mount capacity, it can change between mounts due to quotas (ZFS, BTRFS?)
			if volume.total_capacity < total_capacity {
				volume.total_capacity = total_capacity;
			}

			// This shouldn't change between mounts, but just in case
			if volume.available_capacity > available_capacity {
				volume.available_capacity = available_capacity;
			}

			continue;
		}

		// Assign volume to disk path
		path_to_volume_index.insert(disk_path.into_os_string(), volumes.len());

		let mut name = disk_name.to_string_lossy().to_string();
		if name.replace(char::REPLACEMENT_CHARACTER, "") == "" {
			name = "Unknown".to_string()
		}

		volumes.push(Volume {
			name,
			disk_type: if disk.is_removable() {
				DiskType::Removable
			} else {
				match disk.kind() {
					sysinfo::DiskKind::SSD => DiskType::SSD,
					sysinfo::DiskKind::HDD => DiskType::HDD,
					_ => DiskType::Removable,
				}
			},
			file_system,
			mount_points: vec![mount_point],
			total_capacity,
			available_capacity,
			is_root_filesystem,
		});
	}

	volumes
}

#[cfg(target_os = "macos")]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ImageSystemEntity {
	mount_point: Option<String>,
}

#[cfg(target_os = "macos")]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ImageInfo {
	system_entities: Vec<ImageSystemEntity>,
}

#[cfg(target_os = "macos")]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct HDIUtilInfo {
	images: Vec<ImageInfo>,
}

#[cfg(not(target_os = "linux"))]
pub async fn get_volumes() -> Vec<Volume> {
	use futures::future;
	use tokio::process::Command;

	let mut sys = sys_guard().lock().await;
	sys.refresh_disks_list();

	// Ignore mounted DMGs
	#[cfg(target_os = "macos")]
	let dmgs = &Command::new("hdiutil")
		.args(["info", "-plist"])
		.output()
		.await
		.map_err(|err| error!("Failed to execute hdiutil: {err:#?}"))
		.ok()
		.and_then(|wmic_process| {
			use std::str::FromStr;

			if wmic_process.status.success() {
				let info: Result<HDIUtilInfo, _> = plist::from_bytes(&wmic_process.stdout);
				match info {
					Err(err) => {
						error!("Failed to parse hdiutil output: {err:#?}");
						None
					}
					Ok(info) => Some(
						info.images
							.into_iter()
							.flat_map(|image| image.system_entities)
							.flat_map(|entity: ImageSystemEntity| entity.mount_point)
							.flat_map(|mount_point| PathBuf::from_str(mount_point.as_str()))
							.collect::<std::collections::HashSet<_>>(),
					),
				}
			} else {
				error!("Command hdiutil return error");
				None
			}
		});

	future::join_all(sys.disks().iter().map(|disk| async {
		#[cfg(not(windows))]
		let disk_name = disk.name();
		let mount_point = disk.mount_point().to_path_buf();

		#[cfg(windows)]
		let Ok((disk_name, mount_point)) = ({
			use normpath::PathExt;
			mount_point
				.normalize_virtually()
				.map(|p| (p.localize_name().to_os_string(), p.into_path_buf()))
		}) else {
			return None;
		};

		#[cfg(target_os = "macos")]
		{
			// Ignore mounted DMGs
			if dmgs
				.as_ref()
				.map(|dmgs| dmgs.contains(&mount_point))
				.unwrap_or(false)
			{
				return None;
			}

			if !(mount_point.starts_with("/Volumes") || mount_point.starts_with("/System/Volumes"))
			{
				return None;
			}
		}

		#[cfg(windows)]
		let mut total_capacity;
		#[cfg(not(windows))]
		let total_capacity;

		total_capacity = disk.total_space();
		let available_capacity = disk.available_space();
		let is_root_filesystem = mount_point.is_absolute() && mount_point.parent().is_none();

		// Fix broken google drive partition size in Windows
		#[cfg(windows)]
		if total_capacity < available_capacity && is_root_filesystem {
			// Use available capacity as total capacity in the case we can't get the correct value
			total_capacity = available_capacity;

			let caption = mount_point.to_str();
			if let Some(caption) = caption {
				let mut caption = caption.to_string();

				// Remove path separator from Disk letter
				caption.pop();

				let wmic_output = Command::new("cmd")
					.args([
						"/C",
						&format!("wmic logical disk where Caption='{caption}' get Size"),
					])
					.output()
					.await
					.map_err(|err| error!("Failed to execute hdiutil: {err:#?}"))
					.ok()
					.and_then(|wmic_process| {
						if wmic_process.status.success() {
							String::from_utf8(wmic_process.stdout).ok()
						} else {
							error!("Command wmic return error");
							None
						}
					});

				if let Some(wmic_output) = wmic_output {
					match wmic_output.split("\r\r\n").collect::<Vec<&str>>()[1]
						.to_string()
						.trim()
						.parse::<u64>()
					{
						Err(err) => error!("Failed to parse wmic output: {err:#?}"),
						Ok(n) => total_capacity = n,
					}
				}
			}
		}

		let mut name = disk_name.to_string_lossy().to_string();
		if name.replace(char::REPLACEMENT_CHARACTER, "") == "" {
			name = "Unknown".to_string()
		}

		Some(Volume {
			name,
			disk_type: if disk.is_removable() {
				DiskType::Removable
			} else {
				match disk.kind() {
					sysinfo::DiskKind::SSD => DiskType::SSD,
					sysinfo::DiskKind::HDD => DiskType::HDD,
					_ => DiskType::Removable,
				}
			},
			mount_points: vec![mount_point],
			file_system: String::from_utf8(disk.file_system().to_vec()).ok(),
			total_capacity,
			available_capacity,
			is_root_filesystem,
		})
	}))
	.await
	.into_iter()
	.flatten()
	.collect::<Vec<Volume>>()
}

// pub async fn save_volume(library: &Library) -> Result<(), VolumeError> {
// 	// enter all volumes associate with this client add to db
// 	for volume in get_volumes() {
// 		let params = vec![
// 			disk_type::set(volume.disk_type.map(|t| t.to_string())),
// 			filesystem::set(volume.file_system.clone()),
// 			total_bytes_capacity::set(volume.total_capacity.to_string()),
// 			total_bytes_available::set(volume.available_capacity.to_string()),
// 		];

// 		library
// 			.db
// 			.volume()
// 			.upsert(
// 				node_id_mount_point_name(
// 					library.node_local_id,
// 					volume.mount_point,
// 					volume.name,
// 				),
// 				volume::create(
// 					library.node_local_id,
// 					volume.name,
// 					volume.mount_point,
// 					params.clone(),
// 				),
// 				params,
// 			)
// 			.exec()
// 			.await?;
// 	}
// 	// cleanup: remove all unmodified volumes associate with this client

// 	Ok(())
// }

// #[test]
// fn test_get_volumes() {
//   let volumes = get_volumes()?;
//   dbg!(&volumes);
//   assert!(volumes.len() > 0);
// }
