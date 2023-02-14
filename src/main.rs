use std::{collections::HashMap, net::Ipv4Addr, path::PathBuf, sync::Arc};

use bollard::{Docker, API_DEFAULT_VERSION};
use clap::Parser;
use ephemerish::server::Server;
use russh_keys::{decode_secret_key, key::PublicKey, parse_public_key_base64};

#[derive(Parser)]
struct Args {
	/// The address on which to listen to SSH connections.
	#[arg(
		short = 'a',
		long,
		default_value = "0.0.0.0",
		env = "EPHEMERISH_LISTEN_ADDRESS"
	)]
	pub listen_address: Ipv4Addr,
	/// The port on which to listen to SSH connections.
	#[arg(
		short = 'p',
		long,
		default_value = "2222",
		env = "EPHEMERISH_LISTEN_PORT"
	)]
	pub listen_port: u16,
	/// The socket through which to connect to Docker
	#[arg(
		short = 'd',
		long,
		default_value = "/var/run/docker.sock",
		env = "DOCKER_HOST"
	)]
	pub docker_socket: PathBuf,
	/// The directory to search for authorized keys
	#[arg(
		short = 'k',
		long,
		default_value = "./authorized_keys",
		env = "EPHEMERISH_CLIENT_KEYS_DIR"
	)]
	pub client_keys_dir: PathBuf,
	/// The file holding the server key
	#[arg(
		short = 'K',
		long,
		default_value = "./server.key",
		env = "EPHEMERISH_SERVER_KEY"
	)]
	pub server_key_path: PathBuf,
	/// The password for the server key
	#[arg(short = 'P', long, env = "EPHEMERISH_SERVER_KEY_PASSWORD")]
	pub server_key_password: Option<String>,
}

#[tokio::main]
async fn main() {
	let args = Args::parse();
	let mut russh_config = russh::server::Config::default();

	// Open Docker connection
	let docker = Docker::connect_with_socket(
		args.docker_socket
			.to_str()
			.expect("Docker socket path is invalid utf8???"),
		3,
		API_DEFAULT_VERSION,
	)
	.expect("Couldn't reach Docker");

	// Load the server key
	let server_key = decode_secret_key(
		&tokio::fs::read_to_string(args.server_key_path)
			.await
			.expect("Failed to read server key"),
		args.server_key_password.as_deref(),
	)
	.expect("Server key was formatted incorrectly");
	russh_config.keys.clear();
	russh_config.keys.push(server_key);

	// Load user keys
	let mut user_keys = HashMap::new();
	for entry in std::fs::read_dir(args.client_keys_dir)
		.expect("Failed to list client key files")
		.flatten()
	{
		if entry
			.file_type()
			.expect("Couldn't determine type of entry")
			.is_dir()
		{
			continue;
		}
		if let Some(name) = entry.file_name().to_str() {
			let mut keys: Vec<PublicKey> = vec![];
			for key_line in std::fs::read_to_string(entry.path())
				.expect("Couldn't read user pubkey file")
				.lines()
				.filter(|s| !s.is_empty())
			{
				let key_line = key_line.split_ascii_whitespace().nth(1).unwrap();
				match parse_public_key_base64(key_line) {
					Ok(key) => keys.push(key),
					Err(e) => eprintln!("Parsing pubkey line {key_line} for {name} hit error {e}."),
				}
			}
			user_keys.insert(name.to_string(), keys);
		}
	}

	let server = Server::new(docker, user_keys);
	russh::server::run(Arc::new(russh_config), ("0.0.0.0", 2222), server.clone())
		.await
		.expect("Server failed")
}
