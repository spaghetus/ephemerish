use std::fmt::Display;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SSHError {
	BadLogin,
	SshError(#[from] russh::Error),
}

impl Display for SSHError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{self:?}")
	}
}
