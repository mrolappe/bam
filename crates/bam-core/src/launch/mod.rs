//! The `Launcher` trait (invariant I4's third registry, mirroring
//! `query::lang::LanguageRegistry` and `unpack::UnpackerRegistry`): an
//! emulator backend is anything that can `probe` its own availability and
//! `launch` a request. `capabilities()` **drives selection**, not just
//! reporting — a request needing `directory_volume` must skip a launcher
//! lacking it even when otherwise preferred (§12/§12.1), so vAmiga's weak
//! directory-volume support becomes a routing decision the core makes, not
//! a fact buried in launcher-specific code.

use thiserror::Error;

pub use crate::unpack::Availability;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LauncherCaps {
    pub directory_volume: bool,
    pub uaem_sidecars: bool,
    pub hardfile: bool,
    pub adf: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Capability {
    DirectoryVolume,
    UaemSidecars,
    Hardfile,
    Adf,
}

impl Capability {
    fn name(self) -> &'static str {
        match self {
            Capability::DirectoryVolume => "directory_volume",
            Capability::UaemSidecars => "uaem_sidecars",
            Capability::Hardfile => "hardfile",
            Capability::Adf => "adf",
        }
    }
}

impl LauncherCaps {
    /// Checks that every capability `required` sets is also set here,
    /// naming the first unmet one so a selection failure can say *why*
    /// rather than just "no launcher found".
    fn satisfies(&self, required: &LauncherCaps) -> Result<(), Capability> {
        if required.directory_volume && !self.directory_volume {
            return Err(Capability::DirectoryVolume);
        }
        if required.uaem_sidecars && !self.uaem_sidecars {
            return Err(Capability::UaemSidecars);
        }
        if required.hardfile && !self.hardfile {
            return Err(Capability::Hardfile);
        }
        if required.adf && !self.adf {
            return Err(Capability::Adf);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LaunchRequest {
    pub required: LauncherCaps,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchHandle {
    pub launcher_id: String,
}

#[derive(Debug, Error)]
pub enum LauncherError {
    #[error("launcher '{0}' not registered")]
    UnknownLauncher(String),
    #[error("launcher '{0}' is unavailable")]
    Unavailable(String),
    #[error("no available launcher satisfies required capability '{0}'")]
    CapabilityUnmet(&'static str),
    #[error("no launcher available")]
    NoneAvailable,
}

pub trait Launcher {
    fn id(&self) -> &str;
    /// Is the backend usable here (e.g. found on `PATH` / at a configured
    /// path)?
    fn probe(&self) -> Availability;
    fn capabilities(&self) -> LauncherCaps;
    fn launch(&self, req: &LaunchRequest) -> Result<LaunchHandle, LauncherError>;
}

pub struct LauncherRegistry {
    launchers: Vec<Box<dyn Launcher>>,
}

impl LauncherRegistry {
    pub fn new() -> Self {
        Self {
            launchers: Vec::new(),
        }
    }

    pub fn register(&mut self, launcher: Box<dyn Launcher>) {
        self.launchers.push(launcher);
    }

    /// Config override first (must be registered and probe available, and
    /// its capabilities must satisfy the request), else the first
    /// registered launcher — registration order is preference order — that
    /// is both available and capability-sufficient.
    pub fn select(
        &self,
        req: &LaunchRequest,
        override_id: Option<&str>,
    ) -> Result<&dyn Launcher, LauncherError> {
        if let Some(id) = override_id {
            let l = self
                .launchers
                .iter()
                .find(|l| l.id() == id)
                .ok_or_else(|| LauncherError::UnknownLauncher(id.to_string()))?;
            if l.probe() != Availability::Available {
                return Err(LauncherError::Unavailable(id.to_string()));
            }
            return l
                .capabilities()
                .satisfies(&req.required)
                .map(|()| l.as_ref())
                .map_err(|cap| LauncherError::CapabilityUnmet(cap.name()));
        }

        let mut missing_capability = None;
        for l in &self.launchers {
            if l.probe() != Availability::Available {
                continue;
            }
            match l.capabilities().satisfies(&req.required) {
                Ok(()) => return Ok(l.as_ref()),
                Err(cap) => {
                    missing_capability.get_or_insert(cap);
                }
            }
        }
        match missing_capability {
            Some(cap) => Err(LauncherError::CapabilityUnmet(cap.name())),
            None => Err(LauncherError::NoneAvailable),
        }
    }
}

impl Default for LauncherRegistry {
    fn default() -> Self {
        Self::new()
    }
}
