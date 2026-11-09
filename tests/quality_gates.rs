use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct GateFixture {
    root: tempfile::TempDir,
    log: PathBuf,
}

impl GateFixture {
    fn new(staged: &str, unstaged: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let repo = root.path().join("repo");
        fs::create_dir(&repo).unwrap();
        assert!(
            Command::new("git")
                .args(["init", "--quiet"])
                .current_dir(&repo)
                .status()
                .unwrap()
                .success()
        );
        fs::write(repo.join("content.rs"), staged).unwrap();
        assert!(
            Command::new("git")
                .args(["add", "content.rs"])
                .current_dir(&repo)
                .status()
                .unwrap()
                .success()
        );
        fs::write(repo.join("content.rs"), unstaged).unwrap();
        let mock = root.path().join("bin");
        fs::create_dir(&mock).unwrap();
        let cargo = mock.join("cargo");
        fs::write(
            &cargo,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$GATE_TEST_LOG\"\ncase \"$1\" in fmt) grep -q '^fmt_clean$' content.rs;; clippy) grep -q '^lint_clean$' content.rs;; *) exit 2;; esac\n",
        )
        .unwrap();
        fs::set_permissions(cargo, fs::Permissions::from_mode(0o700)).unwrap();
        let log = root.path().join("checks.log");
        Self { root, log }
    }

    fn repo(&self) -> PathBuf {
        self.root.path().join("repo")
    }

    fn run(&self, bypass: bool) -> Output {
        let path = format!(
            "{}:{}",
            self.root.path().join("bin").display(),
            std::env::var("PATH").unwrap()
        );
        Command::new("sh")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check-staged.sh"))
            .current_dir(self.repo())
            .env("PATH", path)
            .env("GATE_TEST_LOG", &self.log)
            .env("SAGEBRUSH_SKIP_CHECKS", if bypass { "1" } else { "0" })
            .output()
            .unwrap()
    }
}

#[test]
fn checks_exact_staged_content_and_leaves_worktree_unchanged() {
    let fixture = GateFixture::new("fmt_clean\nlint_clean\n", "dirty\n");
    let output = fixture.run(false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(fixture.repo().join("content.rs")).unwrap(),
        "dirty\n"
    );
    let staged = Command::new("git")
        .args(["show", ":content.rs"])
        .current_dir(fixture.repo())
        .output()
        .unwrap();
    assert_eq!(staged.stdout, b"fmt_clean\nlint_clean\n");
    let calls = fs::read_to_string(&fixture.log).unwrap();
    assert!(calls.contains("fmt --all -- --check"));
    assert!(calls.contains("clippy --locked --all-targets --all-features -- -D warnings"));
}

#[test]
fn staged_failure_blocks_even_when_worktree_passes_without_modifying_it() {
    let fixture = GateFixture::new("dirty\n", "fmt_clean\nlint_clean\n");
    let output = fixture.run(false);
    assert!(!output.status.success());
    assert_eq!(
        fs::read_to_string(fixture.repo().join("content.rs")).unwrap(),
        "fmt_clean\nlint_clean\n"
    );
    assert!(!output.stderr.is_empty());
}

#[test]
fn unavailable_check_blocks_commit() {
    let fixture = GateFixture::new("fmt_clean\nlint_clean\n", "fmt_clean\nlint_clean\n");
    let cargo = fixture.root.path().join("bin/cargo");
    fs::write(cargo, "#!/bin/sh\nexit 127\n").unwrap();
    let output = fixture.run(false);
    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
}

#[test]
fn staged_lint_failure_blocks_after_format_succeeds() {
    let fixture = GateFixture::new("fmt_clean\n", "fmt_clean\nlint_clean\n");
    let output = fixture.run(false);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("lint check failed"));
    let calls = fs::read_to_string(&fixture.log).unwrap();
    assert!(calls.contains("fmt --all -- --check"));
    assert!(calls.contains("clippy --locked"));
}

#[test]
fn explicit_bypass_skips_checks() {
    let fixture = GateFixture::new("dirty\n", "dirty\n");
    let output = fixture.run(true);
    assert!(output.status.success());
    assert!(!fixture.log.exists());
}

#[test]
fn ci_checks_do_not_honor_commit_bypass_and_stop_on_each_failure() {
    let fixture = GateFixture::new("fmt_clean\nlint_clean\n", "dirty\n");
    let cargo = fixture.root.path().join("bin/cargo");
    fs::write(
        &cargo,
        "#!/bin/sh\nprintf '%s\\n' \"$1\" >> \"$GATE_TEST_LOG\"\n[ \"$1\" != \"$GATE_TEST_FAIL\" ]\n",
    )
    .unwrap();
    let path = format!(
        "{}:{}",
        fixture.root.path().join("bin").display(),
        std::env::var("PATH").unwrap()
    );
    let checks = ["fmt", "clippy", "test", "build"];
    for (index, failing) in checks.iter().enumerate() {
        if fixture.log.exists() {
            fs::remove_file(&fixture.log).unwrap();
        }
        let output = Command::new("sh")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check-ci.sh"))
            .current_dir(fixture.repo())
            .env("PATH", &path)
            .env("GATE_TEST_LOG", &fixture.log)
            .env("GATE_TEST_FAIL", failing)
            .env("SAGEBRUSH_SKIP_CHECKS", "1")
            .output()
            .unwrap();
        assert!(!output.status.success(), "failed {failing} must block CI");
        let calls = fs::read_to_string(&fixture.log).unwrap();
        assert_eq!(calls.lines().collect::<Vec<_>>(), checks[..=index]);
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn ci_checks_run_all_required_commands_without_mutating_the_worktree() {
    let fixture = GateFixture::new("fmt_clean\nlint_clean\n", "dirty\n");
    let cargo = fixture.root.path().join("bin/cargo");
    fs::write(
        &cargo,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$GATE_TEST_LOG\"\n",
    )
    .unwrap();
    let path = format!(
        "{}:{}",
        fixture.root.path().join("bin").display(),
        std::env::var("PATH").unwrap()
    );
    let output = Command::new("sh")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check-ci.sh"))
        .current_dir(fixture.repo())
        .env("PATH", path)
        .env("GATE_TEST_LOG", &fixture.log)
        .env("SAGEBRUSH_SKIP_CHECKS", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        fs::read_to_string(&fixture.log).unwrap(),
        "fmt --all -- --check\nclippy --locked --all-targets --all-features -- -D warnings\ntest --locked --all-targets --all-features\nbuild --locked\n"
    );
    assert_eq!(
        fs::read_to_string(fixture.repo().join("content.rs")).unwrap(),
        "dirty\n"
    );
}

#[test]
fn ci_checks_fail_when_cargo_is_unavailable_even_with_commit_bypass() {
    let fixture = GateFixture::new("fmt_clean\nlint_clean\n", "dirty\n");
    let empty_path = fixture.root.path().join("empty-bin");
    fs::create_dir(&empty_path).unwrap();
    let output = Command::new("/bin/sh")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check-ci.sh"))
        .current_dir(fixture.repo())
        .env("PATH", empty_path)
        .env("SAGEBRUSH_SKIP_CHECKS", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("formatting check failed"));
}

#[test]
fn pull_request_and_release_workflow_depend_on_the_ci_checks() {
    let workflow = include_str!("../.github/workflows/ci.yml");
    let (checks, release) = workflow.split_once("\n  release-gate:\n").unwrap();
    assert!(checks.lines().any(|line| line.trim() == "pull_request:"));
    assert!(
        checks
            .lines()
            .any(|line| line.trim() == "run: sh scripts/check-ci.sh")
    );
    assert!(!checks.contains("continue-on-error: true"));
    assert!(release.lines().any(|line| line.trim() == "needs: checks"));
    assert!(!release.contains("always()"));
    assert!(!release.contains("continue-on-error: true"));
}
