use crate::{
	api::{
		notifications::{Notification, NotificationData, NotificationId},
		CoreEvent,
	},
	env,
	location::file_path_helper::{file_path_to_full_path, IsolatedFilePathData},
	notifications,
	object::{media::thumbnail::get_indexed_thumbnail_path, orphan_remover::OrphanRemoverActor},
	prisma::{file_path, location, PrismaClient},
	sync,
	util::{db::maybe_missing, error::FileIOError},
	Node,
};

use sd_p2p::spacetunnel::Identity;
use sd_prisma::prisma::notification;

use std::{
	collections::HashMap,
	fmt::{Debug, Formatter},
	path::{Path, PathBuf},
	sync::Arc,
};

use chrono::{DateTime, Utc};
use tokio::{fs, io, sync::broadcast, sync::RwLock};
use tracing::warn;
use uuid::Uuid;

use super::{LibraryConfig, LibraryManagerError};

// TODO: Finish this
// pub enum LibraryNew {
// 	InitialSync,
// 	Encrypted,
// 	Loaded(LoadedLibrary),
//  Deleting,
// }

pub struct Library {
	/// id holds the ID of the current library.
	pub id: Uuid,
	/// config holds the configuration of the current library.
	/// KEEP PRIVATE: Access through `Self::config` method.
	config: RwLock<LibraryConfig>,
	/// db holds the database client for the current library.
	pub db: Arc<PrismaClient>,
	pub sync: Arc<sync::Manager>,
	/// key manager that provides encryption keys to functions that require them
	// pub key_manager: Arc<KeyManager>,
	/// p2p identity
	pub identity: Arc<Identity>,
	pub orphan_remover: OrphanRemoverActor,
	// The UUID which matches `config.instance_id`'s primary key.
	pub instance_uuid: Uuid,

	notifications: notifications::Notifications,
	pub env: Arc<crate::env::Env>,

	// Look, I think this shouldn't be here but our current invalidation system needs it.
	// TODO(@Oscar): Get rid of this with the new invalidation system.
	event_bus_tx: broadcast::Sender<CoreEvent>,
}

impl Debug for Library {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		// Rolling out this implementation because `NodeContext` contains a DynJob which is
		// troublesome to implement Debug trait
		f.debug_struct("LibraryContext")
			.field("id", &self.id)
			.field("instance_uuid", &self.instance_uuid)
			.field("config", &self.config)
			.field("db", &self.db)
			.finish()
	}
}

impl Library {
	pub async fn new(
		id: Uuid,
		config: LibraryConfig,
		instance_uuid: Uuid,
		identity: Arc<Identity>,
		db: Arc<PrismaClient>,
		node: &Arc<Node>,
		sync: Arc<sync::Manager>,
	) -> Arc<Self> {
		Arc::new(Self {
			id,
			config: RwLock::new(config),
			sync,
			db: db.clone(),
			// key_manager,
			identity,
			orphan_remover: OrphanRemoverActor::spawn(db),
			notifications: node.notifications.clone(),
			instance_uuid,
			env: node.env.clone(),
			event_bus_tx: node.event_bus.0.clone(),
		})
	}

	pub async fn config(&self) -> LibraryConfig {
		self.config.read().await.clone()
	}

	pub async fn update_config(
		&self,
		update_fn: impl FnOnce(&mut LibraryConfig),
		config_path: impl AsRef<Path>,
	) -> Result<(), LibraryManagerError> {
		let mut config = self.config.write().await;

		update_fn(&mut config);

		config.save(config_path).await.map_err(Into::into)
	}

	// TODO: Remove this once we replace the old invalidation system
	pub(crate) fn emit(&self, event: CoreEvent) {
		if let Err(e) = self.event_bus_tx.send(event) {
			warn!("Error sending event to event bus: {e:?}");
		}
	}

	pub async fn thumbnail_exists(&self, node: &Node, cas_id: &str) -> Result<bool, FileIOError> {
		let thumb_path = get_indexed_thumbnail_path(node, cas_id, self.id);

		match fs::metadata(&thumb_path).await {
			Ok(_) => Ok(true),
			Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
			Err(e) => Err(FileIOError::from((thumb_path, e))),
		}
	}

	/// Returns the full path of a file
	pub async fn get_file_paths(
		&self,
		ids: Vec<file_path::id::Type>,
	) -> Result<HashMap<file_path::id::Type, Option<PathBuf>>, LibraryManagerError> {
		let mut out = ids
			.iter()
			.copied()
			.map(|id| (id, None))
			.collect::<HashMap<_, _>>();

		out.extend(
			self.db
				.file_path()
				.find_many(vec![
					// TODO(N): This isn't gonna work with removable media and this will likely permanently break if the DB is restored from a backup.
					file_path::location::is(vec![location::instance_id::equals(Some(
						self.config().await.instance_id,
					))]),
					file_path::id::in_vec(ids),
				])
				.select(file_path_to_full_path::select())
				.exec()
				.await?
				.into_iter()
				.flat_map(|file_path| {
					let location = maybe_missing(&file_path.location, "file_path.location")?;

					Ok::<_, LibraryManagerError>((
						file_path.id,
						location
							.path
							.as_ref()
							.map(|location_path| {
								IsolatedFilePathData::try_from((location.id, &file_path))
									.map(|data| Path::new(&location_path).join(data))
							})
							.transpose()?,
					))
				}),
		);

		Ok(out)
	}

	/// Create a new notification which will be stored into the DB and emitted to the UI.
	pub async fn emit_notification(&self, data: NotificationData, expires: Option<DateTime<Utc>>) {
		let result = match self
			.db
			.notification()
			.create(
				match rmp_serde::to_vec(&data).map_err(|err| err.to_string()) {
					Ok(data) => data,
					Err(err) => {
						warn!(
							"Failed to serialize notification data for library '{}': {}",
							self.id, err
						);
						return;
					}
				},
				expires
					.map(|e| vec![notification::expires_at::set(Some(e.fixed_offset()))])
					.unwrap_or_default(),
			)
			.exec()
			.await
		{
			Ok(result) => result,
			Err(err) => {
				warn!(
					"Failed to create notification in library '{}': {}",
					self.id, err
				);
				return;
			}
		};

		self.notifications._internal_send(Notification {
			id: NotificationId::Library(self.id, result.id as u32),
			data,
			read: false,
			expires,
		});
	}
}
