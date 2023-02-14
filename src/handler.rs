use async_trait::async_trait;
use russh::server::{Auth, Session};
use russh_keys::key::PublicKey;

use super::server::Server;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

pub struct Handler {
	/// Owned clone of server; shares memory with the main instance thanks to Arc
	pub server: Server,
	/// The username of the user for which this handler applies
	pub name: Option<String>,
	/// The IP address of the user
	pub ip: Option<SocketAddr>,
	/// A handle to the user's container so it never expires while a connection is active
	pub handle: Option<Arc<()>>,
	/// The SSH client connection for this user
	pub client: Option<()>,
}

#[async_trait]
impl russh::server::Handler for Handler {
	type Error = crate::error::SSHError;

	/// When the client disconnects, if they have a
	async fn disconnected(self, session: Session) -> Result<(Self, Session), Self::Error> {
		if let Some(name) = self.name.clone() {
			self.server.last_closure.insert(name, Instant::now());
		}
		Ok((self, session))
	}

	async fn auth_publickey(
		mut self,
		user: &str,
		public_key: &PublicKey,
	) -> Result<(Self, Auth), Self::Error> {
		println!(
			"Login attempt by {user} with key {}",
			public_key.fingerprint()
		);
		if let Some(keys) = self.server.users.get(user) {
			if keys.iter().any(|key| key == public_key) {
				self.name = Some(user.to_string());
				self.handle = Some(
					self.server
						.connections
						.entry(user.to_string())
						.or_insert(Arc::new(()))
						.clone(),
				);
				println!("{user} successful login!");
				return Ok((self, Auth::Accept));
			}
		}

		println!("{user} rejected.");
		Ok((
			self,
			Auth::Reject {
				proceed_with_methods: None,
			},
		))
	}

	async fn auth_succeeded(self, session: Session) -> Result<(Self, Session), Self::Error> {
		Ok((self, session))
	}
}
