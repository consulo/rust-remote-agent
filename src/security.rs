// Copyright 2013-2026 consulo.io
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject;
use sha2::{Digest, Sha256};
use tonic::service::Interceptor;
use tonic::{Request, Status};

pub struct TlsMaterial {
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
    /// SHA-256 of the leaf certificate, e.g. "AB:CD:..."
    pub fingerprint: String,
    pub generated_at: Option<PathBuf>,
}

/// Loads the given certificate and key, or creates a self-signed pair once and keeps it under `state_dir`.
pub fn load_tls(cert: Option<&Path>, key: Option<&Path>, state_dir: &Path, names: Vec<String>) -> Result<TlsMaterial, String> {
    if let (Some(cert), Some(key)) = (cert, key) {
        let cert_pem = fs::read(cert).map_err(|e| format!("Failed to read TLS certificate '{}': {}", cert.display(), e))?;
        let key_pem = fs::read(key).map_err(|e| format!("Failed to read TLS key '{}': {}", key.display(), e))?;
        let fingerprint = fingerprint_of_pem(&cert_pem)?;
        return Ok(TlsMaterial {
            cert_pem,
            key_pem,
            fingerprint,
            generated_at: None,
        });
    }

    let dir = state_dir.join("tls");
    let cert_path = dir.join("agent.crt");
    let key_path = dir.join("agent.key");
    if !cert_path.exists() || !key_path.exists() {
        generate(&dir, &cert_path, &key_path, names)?;
    }
    let cert_pem = fs::read(&cert_path).map_err(|e| format!("Failed to read '{}': {}", cert_path.display(), e))?;
    let key_pem = fs::read(&key_path).map_err(|e| format!("Failed to read '{}': {}", key_path.display(), e))?;
    let fingerprint = fingerprint_of_pem(&cert_pem)?;
    Ok(TlsMaterial {
        cert_pem,
        key_pem,
        fingerprint,
        generated_at: Some(dir),
    })
}

fn generate(dir: &Path, cert_path: &Path, key_path: &Path, names: Vec<String>) -> Result<(), String> {
    let generated = rcgen::generate_simple_self_signed(names).map_err(|e| format!("Failed to create a TLS certificate: {}", e))?;
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create '{}': {}", dir.display(), e))?;
    write_private(key_path, generated.signing_key.serialize_pem().as_bytes())?;
    fs::write(cert_path, generated.cert.pem()).map_err(|e| format!("Failed to write '{}': {}", cert_path.display(), e))
}

#[cfg(unix)]
fn write_private(path: &Path, content: &[u8]) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("Failed to write '{}': {}", path.display(), e))?;
    file.write_all(content).map_err(|e| format!("Failed to write '{}': {}", path.display(), e))
}

#[cfg(not(unix))]
fn write_private(path: &Path, content: &[u8]) -> Result<(), String> {
    fs::write(path, content).map_err(|e| format!("Failed to write '{}': {}", path.display(), e))
}

fn fingerprint_of_pem(pem: &[u8]) -> Result<String, String> {
    let cert = CertificateDer::pem_slice_iter(pem)
        .next()
        .ok_or_else(|| "The TLS certificate file contains no certificate".to_string())?
        .map_err(|e| format!("Invalid TLS certificate: {:?}", e))?;
    let digest = Sha256::digest(cert.as_ref());
    Ok(digest.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(":"))
}

/// Rejects calls without `authorization: Bearer <token>` when a token is configured.
#[derive(Clone)]
pub struct TokenCheck {
    expected: Option<Arc<[u8]>>,
}

impl TokenCheck {
    pub fn new(token: Option<String>) -> Self {
        TokenCheck {
            expected: token.filter(|t| !t.is_empty()).map(|t| Arc::from(format!("Bearer {}", t).into_bytes())),
        }
    }
}

impl Interceptor for TokenCheck {
    fn call(&mut self, request: Request<()>) -> Result<Request<()>, Status> {
        let Some(expected) = &self.expected else {
            return Ok(request);
        };
        let given = request.metadata().get("authorization").map(|v| v.as_bytes()).unwrap_or_default();
        if constant_time_eq(given, expected) {
            Ok(request)
        } else {
            Err(Status::unauthenticated("A valid agent token is required"))
        }
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter().zip(right).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}
