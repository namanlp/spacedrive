use std::{collections::HashMap, env, str::FromStr};

use sd_p2p::Metadata;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::node::Platform;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
pub struct PeerMetadata {
	pub name: String,
	pub operating_system: Option<OperatingSystem>,
	pub version: Option<String>,
}

impl Metadata for PeerMetadata {
	fn to_hashmap(self) -> HashMap<String, String> {
		let mut map = HashMap::with_capacity(5);
		map.insert("name".to_owned(), self.name);
		if let Some(os) = self.operating_system {
			map.insert("os".to_owned(), os.to_string());
		}
		if let Some(version) = self.version {
			map.insert("version".to_owned(), version);
		}
		map
	}

	fn from_hashmap(data: &HashMap<String, String>) -> Result<Self, String>
	where
		Self: Sized,
	{
		Ok(Self {
			name: data
				.get("name")
				.ok_or_else(|| {
					"DNS record for field 'name' missing. Unable to decode 'PeerMetadata'!"
						.to_owned()
				})?
				.to_owned(),
			operating_system: data
				.get("os")
				.map(|os| os.parse().map_err(|_| "Unable to parse 'OperationSystem'!"))
				.transpose()?,
			version: data.get("version").map(|v| v.to_owned()),
		})
	}
}

/// Represents the operating system which the remote peer is running.
/// This is not used internally and predominantly is designed to be used for display purposes by the embedding application.
#[derive(Debug, Clone, Type, Serialize, Deserialize)]
pub enum OperatingSystem {
	Windows,
	Linux,
	MacOS,
	Ios,
	Android,
	Other(String),
}

// TODO: Should `Platform` and `OperatingSystem` be merged into one?
impl From<Platform> for OperatingSystem {
	fn from(platform: Platform) -> Self {
		match platform {
			Platform::Unknown => OperatingSystem::Other("Unknown".into()),
			Platform::Windows => OperatingSystem::Windows,
			Platform::Linux => OperatingSystem::Linux,
			Platform::MacOS => OperatingSystem::MacOS,
			Platform::IOS => OperatingSystem::Ios,
			Platform::Android => OperatingSystem::Android,
		}
	}
}

impl OperatingSystem {
	pub fn get_os() -> Self {
		match env::consts::OS {
			"windows" => OperatingSystem::Windows,
			"macos" => OperatingSystem::MacOS,
			"linux" => OperatingSystem::Linux,
			"ios" => OperatingSystem::Ios,
			"android" => OperatingSystem::Android,
			platform => OperatingSystem::Other(platform.into()),
		}
	}
}

impl ToString for OperatingSystem {
	fn to_string(&self) -> String {
		match self {
			OperatingSystem::Windows => "Windows".into(),
			OperatingSystem::Linux => "Linux".into(),
			OperatingSystem::MacOS => "MacOS".into(),
			OperatingSystem::Ios => "IOS".into(),
			OperatingSystem::Android => "Android".into(),
			OperatingSystem::Other(s) => {
				let mut chars = s.chars();
				chars.next();
				chars.as_str().to_string()
			}
		}
	}
}

impl FromStr for OperatingSystem {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut chars = s.chars();
		match chars.next() {
			Some('W') => Ok(OperatingSystem::Windows),
			Some('L') => Ok(OperatingSystem::Linux),
			Some('M') => Ok(OperatingSystem::MacOS),
			Some('I') => Ok(OperatingSystem::Ios),
			Some('A') => Ok(OperatingSystem::Android),
			_ => Ok(OperatingSystem::Other(s.to_owned())),
		}
	}
}
