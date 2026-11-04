//! Netzzugriffe. Nur `https://`, damit nichts unverschlüsselt geladen wird.

use std::path::Path;

use crate::error::{Result, TermaError};

pub trait Http: Send + Sync {
    fn get_string(&self, url: &str) -> Result<String>;
    /// Für Antworten, die kein Text sind (zum Beispiel `.zsync`-Dateien).
    fn get_bytes(&self, url: &str) -> Result<Vec<u8>>;
    fn download(&self, url: &str, dest: &Path) -> Result<()>;
}

pub struct UreqHttp {
    agent: ureq::Agent,
}

impl UreqHttp {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .user_agent(concat!("terma/", env!("CARGO_PKG_VERSION")))
            // Auch für Weiterleitungen. Für den Inhalt selbst keine Grenze, AppImages dürfen groß sein.
            .https_only(true)
            .timeout_connect(Some(std::time::Duration::from_secs(15)))
            .timeout_recv_response(Some(std::time::Duration::from_secs(30)))
            .build();
        UreqHttp { agent: config.into() }
    }

    fn get(&self, url: &str) -> Result<ureq::http::Response<ureq::Body>> {
        if !url.starts_with("https://") {
            return Err(TermaError::Network(format!("nur https erlaubt: {url}")));
        }
        self.agent.get(url).call().map_err(|e| TermaError::Network(e.to_string()))
    }
}

impl Default for UreqHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl Http for UreqHttp {
    fn get_string(&self, url: &str) -> Result<String> {
        self.get(url)?
            .body_mut()
            .with_config()
            .limit(64 * 1024 * 1024)
            .read_to_string()
            .map_err(|e| TermaError::Network(e.to_string()))
    }

    fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        self.get(url)?
            .body_mut()
            .with_config()
            .limit(64 * 1024 * 1024)
            .read_to_vec()
            .map_err(|e| TermaError::Network(e.to_string()))
    }

    fn download(&self, url: &str, dest: &Path) -> Result<()> {
        let mut response = self.get(url)?;
        let mut file = std::fs::File::create(dest)?;
        let mut reader = response.body_mut().with_config().limit(u64::MAX).reader();
        std::io::copy(&mut reader, &mut file).map_err(|e| TermaError::Network(e.to_string()))?;
        Ok(())
    }
}

#[cfg(any(test, feature = "test-util"))]
pub use fake::FakeHttp;

#[cfg(any(test, feature = "test-util"))]
mod fake {
    use super::Http;
    use crate::error::{Result, TermaError};
    use std::collections::HashMap;
    use std::path::Path;

    #[derive(Default)]
    pub struct FakeHttp {
        bodies: HashMap<String, Vec<u8>>,
    }

    impl FakeHttp {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn on(mut self, url: &str, body: impl AsRef<[u8]>) -> Self {
            self.bodies.insert(url.to_string(), body.as_ref().to_vec());
            self
        }

        fn body(&self, url: &str) -> Result<&Vec<u8>> {
            self.bodies.get(url).ok_or_else(|| TermaError::Network(format!("kein Fake für {url}")))
        }
    }

    impl Http for FakeHttp {
        fn get_string(&self, url: &str) -> Result<String> {
            Ok(String::from_utf8_lossy(self.body(url)?).into_owned())
        }

        fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
            Ok(self.body(url)?.clone())
        }

        fn download(&self, url: &str, dest: &Path) -> Result<()> {
            std::fs::write(dest, self.body(url)?)?;
            Ok(())
        }
    }
}
