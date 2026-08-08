use bam_core::launch::{
    Availability, LaunchHandle, LaunchRequest, Launcher, LauncherCaps, LauncherError,
    LauncherRegistry,
};

struct FakeLauncher {
    id: &'static str,
    availability: Availability,
    caps: LauncherCaps,
}

impl Launcher for FakeLauncher {
    fn id(&self) -> &str {
        self.id
    }

    fn probe(&self) -> Availability {
        self.availability.clone()
    }

    fn capabilities(&self) -> LauncherCaps {
        self.caps
    }

    fn launch(&self, _req: &LaunchRequest) -> Result<LaunchHandle, LauncherError> {
        Ok(LaunchHandle {
            launcher_id: self.id.to_string(),
        })
    }
}

fn available(id: &'static str, caps: LauncherCaps) -> FakeLauncher {
    FakeLauncher {
        id,
        availability: Availability::Available,
        caps,
    }
}

fn full_caps() -> LauncherCaps {
    LauncherCaps {
        directory_volume: true,
        uaem_sidecars: true,
        hardfile: true,
        adf: true,
    }
}

#[test]
fn registry_picks_by_configured_preference() {
    let mut reg = LauncherRegistry::new();
    reg.register(Box::new(available("preferred", full_caps())));
    reg.register(Box::new(available("second", full_caps())));

    let chosen = reg.select(&LaunchRequest::default(), None).unwrap();
    assert_eq!(chosen.id(), "preferred");
}

#[test]
fn unavailable_launcher_is_never_selected() {
    let mut reg = LauncherRegistry::new();
    reg.register(Box::new(FakeLauncher {
        id: "broken",
        availability: Availability::Unavailable {
            reason: "binary not found".to_string(),
        },
        caps: full_caps(),
    }));
    reg.register(Box::new(available("working", full_caps())));

    let chosen = reg.select(&LaunchRequest::default(), None).unwrap();
    assert_eq!(chosen.id(), "working");
}

#[test]
fn request_needing_directory_volume_skips_launcher_lacking_it_even_when_preferred() {
    let mut reg = LauncherRegistry::new();
    reg.register(Box::new(available(
        "preferred-no-dirvol",
        LauncherCaps {
            directory_volume: false,
            ..full_caps()
        },
    )));
    reg.register(Box::new(available("fallback-with-dirvol", full_caps())));

    let req = LaunchRequest {
        required: LauncherCaps {
            directory_volume: true,
            ..Default::default()
        },
    };
    let chosen = reg.select(&req, None).unwrap();
    assert_eq!(chosen.id(), "fallback-with-dirvol");
}

#[test]
fn no_launcher_satisfying_request_names_the_missing_capability() {
    let mut reg = LauncherRegistry::new();
    reg.register(Box::new(available(
        "no-hardfile",
        LauncherCaps {
            hardfile: false,
            ..full_caps()
        },
    )));

    let req = LaunchRequest {
        required: LauncherCaps {
            hardfile: true,
            ..Default::default()
        },
    };
    let msg = match reg.select(&req, None) {
        Ok(_) => panic!("expected no launcher to satisfy the request"),
        Err(err) => err.to_string(),
    };
    assert!(
        msg.contains("hardfile"),
        "message should name the unmet capability: {msg}"
    );
}

#[test]
fn config_override_wins_over_preference_order() {
    let mut reg = LauncherRegistry::new();
    reg.register(Box::new(available("preferred", full_caps())));
    reg.register(Box::new(available("second", full_caps())));

    let chosen = reg
        .select(&LaunchRequest::default(), Some("second"))
        .unwrap();
    assert_eq!(chosen.id(), "second");
}

#[test]
fn overriding_to_an_unavailable_launcher_errors_clearly() {
    let mut reg = LauncherRegistry::new();
    reg.register(Box::new(FakeLauncher {
        id: "broken",
        availability: Availability::Unavailable {
            reason: "binary not found".to_string(),
        },
        caps: full_caps(),
    }));
    reg.register(Box::new(available("working", full_caps())));

    match reg.select(&LaunchRequest::default(), Some("broken")) {
        Err(LauncherError::Unavailable(id)) => assert_eq!(id, "broken"),
        Err(other) => panic!("expected Unavailable, got {other:?}"),
        Ok(_) => panic!("expected an error"),
    }
}
