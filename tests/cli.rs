//! Smoke tests for the binary surface: what a user gets from a fresh install.

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_prints_the_crate_version() {
    Command::cargo_bin("rigger")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_describes_the_product() {
    Command::cargo_bin("rigger")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("One seat for all your projects and tasks"));
}
