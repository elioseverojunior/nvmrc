//! Which credentials the real clients send: `NVM_AUTH_HEADER` to every host
//! the node clients reach, `NVMRC_NPM_AUTH_HEADER` to the registry host only.

use super::{node_auth_header, registry_auth_header};
use crate::fakes::FakeEnv;

fn env() -> FakeEnv {
    FakeEnv::default()
        .with_var("NVM_AUTH_HEADER", "Bearer node")
        .with_var("NVMRC_NPM_AUTH_HEADER", "Bearer npm")
}

#[test]
fn the_registry_header_is_the_npm_one_scoped_to_the_registry_host() {
    let mirror = env().with_var("NVMRC_NPM_REGISTRY", "https://mirror.test/npm/");
    let header = registry_auth_header(&mirror).expect("a registry header");
    assert_eq!(header.value, "Bearer npm");
    assert_eq!(header.host.as_deref(), Some("mirror.test"));
    let default = registry_auth_header(&env()).expect("a header for the default registry");
    assert_eq!(default.host.as_deref(), Some("registry.npmjs.org"));
}

#[test]
fn the_registry_never_gets_the_node_header() {
    let env = FakeEnv::default().with_var("NVM_AUTH_HEADER", "Bearer node");
    assert!(registry_auth_header(&env).is_none());
}

#[test]
fn an_invalid_registry_gets_no_header() {
    for registry in ["http://mirror.test", "not a url"] {
        let env = env().with_var("NVMRC_NPM_REGISTRY", registry);
        assert!(registry_auth_header(&env).is_none(), "{registry}");
    }
}

#[test]
fn the_node_clients_keep_their_header_unscoped() {
    let header = node_auth_header(&env()).expect("a node header");
    assert_eq!(header.value, "Bearer node");
    assert_eq!(header.host, None);
    let npm_only = FakeEnv::default().with_var("NVMRC_NPM_AUTH_HEADER", "Bearer npm");
    assert!(node_auth_header(&npm_only).is_none());
}
