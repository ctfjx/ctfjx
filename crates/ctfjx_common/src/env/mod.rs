use std::{env, fs};

use dotenvy::dotenv;
use thiserror::Error;
use validator::Validate;

#[derive(Error, Debug)]
pub enum EnvError {
    #[error("missing env `{0}`")]
    Missing(String),

    #[error("env `{0}` is invalid: {1}")]
    Invalid(String, #[source] validator::ValidationErrors),
}

pub trait ResolveEnv: Sized + Default + Validate {
    /// Populate the struct fields
    fn populate(&mut self) -> Result<(), EnvError>;

    /// Creates `Self::default()`, populate then validate
    fn resolve_and_validate() -> Result<Self, EnvError> {
        if let Err(e) = dotenv() {
            debug!("failed to load .env file: {}", e);
        }

        let mut s = Self::default();
        s.populate()?;

        if let Err(e) = s.validate() {
            return Err(EnvError::Invalid("struct".to_string(), e));
        }
        Ok(s)
    }
}

pub fn lookup(k: &str) -> Result<String, EnvError> {
    if k.ends_with("_FILE") {
        return lookup_env_file(k);
    }

    if let Ok(v) = lookup_env(k) {
        return Ok(v);
    }

    lookup_env_file(k)
}

/// Lookup an environment variable
pub fn lookup_env(k: &str) -> Result<String, EnvError> {
    env::var(k).map_err(|_| EnvError::Missing(k.to_string()))
}

/// Lookup env from a file path
pub fn lookup_env_file(k: &str) -> Result<String, EnvError> {
    let file_key = if k.ends_with("_FILE") {
        k.to_string()
    } else {
        format!("{}_FILE", k)
    };

    let env_path = lookup_env(&file_key)?;
    fs::read_to_string(&env_path).map_err(|e| EnvError::Missing(format!("{}: {}", k, e)))
}
