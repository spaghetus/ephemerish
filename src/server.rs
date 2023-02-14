use bollard::Docker;
use dashmap::DashMap;
use russh_keys::key::PublicKey;
use std::{
	collections::HashMap,
	sync::Arc,
	time::{Duration, Instant},
};

use crate::handler::Handler;

#[derive(Clone)]
pub struct Server {
	/// The connection to the Docker daemon.
	pub(crate) docker: Arc<Docker>,
	/// The expiration time for a container after a user disconnects.
	pub(crate) expiration_length: Duration,
	/// The amount of active connections for each user's container.
	/// Implemented as an Arc<()> to ensure that we don't forget to clear it after a connection drops.
	pub(crate) connections: Arc<DashMap<String, Arc<()>>>,
	/// The timestamp of the last connection for each user.
	pub(crate) last_closure: Arc<DashMap<String, Instant>>,
	/// The table of container IDs for each user
	pub(crate) containers: Arc<DashMap<String, String>>,
	/// The table of valid users and their public keys.
	pub(crate) users: Arc<HashMap<String, Vec<PublicKey>>>,
}

impl Server {
	pub fn new(docker: Docker, users: HashMap<String, Vec<PublicKey>>) -> Self {
		Self {
			docker: Arc::new(docker),
			users: Arc::new(users),
			expiration_length: Duration::from_secs(60 * 60),
			connections: Default::default(),
			last_closure: Default::default(),
			containers: Default::default(),
		}
	}
	pub fn connection_count(&self, name: Option<&str>) -> usize {
		if let Some(name) = name {
			if let Some(arc) = self.connections.get(name) {
				std::sync::Arc::<()>::strong_count(arc.value()) - 1
			} else {
				0
			}
		} else {
			self.connections
				.iter()
				.map(|v| std::sync::Arc::<()>::strong_count(v.value()) - 1)
				.sum()
		}
	}

	pub fn expiration_time(&self, name: &str) -> Option<Instant> {
		let connections = self.connection_count(Some(name));
		if connections > 0 {
			return None;
		}
		self.last_closure
			.get(name)
			.map(|timestamp| *timestamp.value() + self.expiration_length)
	}

	pub async fn expire_user(&self, name: &str, force: bool) {
		if let Some(id) = self.containers.get(name) {
			if force
				|| self
					.expiration_time(name)
					.map(|time| time < Instant::now())
					.unwrap_or(false)
			{
				if let Err(e) = self.docker.remove_container(id.value(), None).await {
					eprintln!("Stopping {name}'s container failed with {e}");
				}
			}
		}
	}

	pub fn check_key(&self, name: &str, attempt_key: &PublicKey) -> bool {
		if let Some(keys) = self.users.get(name) {
			for key in keys {
				if key == attempt_key {
					return true;
				}
			}
		}
		false
	}
}

impl russh::server::Server for Server {
	type Handler = Handler;

	fn new_client(&mut self, peer_addr: Option<std::net::SocketAddr>) -> Self::Handler {
		println!("Connection from {peer_addr:?}");
		Handler {
			server: self.clone(),
			name: None,
			ip: peer_addr,
			handle: None,
			client: None,
		}
	}
}
