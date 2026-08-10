use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

#[test]
fn timeout_terminates_the_actual_application_process() {
    let temp = DogfoodTestDir::new("timeout");
    let fake_bin = temp.path().join("bin");
    fs::create_dir(&fake_bin).expect("fake bin directory should be created");
    let app = temp.path().join("native_dogfood");
    let pid_file = temp.path().join("app.pid");
    let terminated_file = temp.path().join("app.terminated");
    write_executable(
        &app,
        r#"#!/usr/bin/env bash
set -euo pipefail
trap 'printf terminated >"$FAKE_TERMINATED_FILE"; exit 0' TERM
printf '%s\n' "$$" >"$FAKE_PID_FILE"
while :; do sleep 0.05; done
"#,
    );
    install_fake_cargo(&fake_bin, &app, &temp.path().join("cargo.invoked"));

    let mut command = run_native_dogfood_script(&temp, &fake_bin, &app);
    command
        .env("FAKE_PID_FILE", &pid_file)
        .env("FAKE_TERMINATED_FILE", &terminated_file)
        .env("RUI_NATIVE_DOGFOOD_POLL_ATTEMPTS", "150")
        .env("RUI_NATIVE_DOGFOOD_POLL_INTERVAL", "0.02");
    let output = output_with_deadline(&mut command, Duration::from_secs(30))
        .expect("native dogfood script should finish within its process deadline");
    assert!(!output.status.success());
    assert!(stderr(&output).contains("timed out waiting for profile-producing exit"));
    assert_pid_recorded(&pid_file, &output, temp.path());
    assert_eq!(
        fs::read_to_string(&terminated_file).expect("termination marker should be written"),
        "terminated"
    );
    assert_process_gone(&pid_file);
}

#[test]
fn timeout_force_kills_an_application_that_ignores_term() {
    let temp = DogfoodTestDir::new("force-kill");
    let fake_bin = temp.path().join("bin");
    fs::create_dir(&fake_bin).expect("fake bin directory should be created");
    let app = temp.path().join("native_dogfood");
    let pid_file = temp.path().join("app.pid");
    write_executable(
        &app,
        r#"#!/usr/bin/env bash
set -euo pipefail
trap '' TERM
printf '%s\n' "$$" >"$FAKE_PID_FILE"
while :; do sleep 0.05; done
"#,
    );
    install_fake_cargo(&fake_bin, &app, &temp.path().join("cargo.invoked"));

    let mut command = run_native_dogfood_script(&temp, &fake_bin, &app);
    command
        .env("FAKE_PID_FILE", &pid_file)
        .env("RUI_NATIVE_DOGFOOD_POLL_ATTEMPTS", "150")
        .env("RUI_NATIVE_DOGFOOD_POLL_INTERVAL", "0.02")
        .env("RUI_NATIVE_DOGFOOD_TERMINATION_GRACE", "0.05");
    let output =
        output_with_deadline(&mut command, Duration::from_secs(30)).unwrap_or_else(|err| {
            force_kill_recorded_process(&pid_file);
            panic!("native dogfood script must finish after its kill grace: {err}");
        });

    assert!(!output.status.success());
    assert!(stderr(&output).contains("timed out waiting for profile-producing exit"));
    assert_pid_recorded(&pid_file, &output, temp.path());
    assert_process_gone(&pid_file);
}

#[test]
fn rejects_case_only_aliases_on_case_insensitive_filesystems() {
    let temp = DogfoodTestDir::new("case-alias");
    let upper = temp.path().join("Profile.json");
    let lower = temp.path().join("profile.json");
    fs::write(&upper, "probe").expect("filesystem probe should be written");
    let case_insensitive = lower.exists();
    fs::remove_file(&upper).expect("filesystem probe should be removed");
    if !case_insensitive {
        return;
    }

    let fake_bin = temp.path().join("bin");
    fs::create_dir(&fake_bin).expect("fake bin directory should be created");
    let app = temp.path().join("native_dogfood");
    write_executable(&app, "#!/usr/bin/env bash\nexit 0\n");
    let cargo_invoked = temp.path().join("cargo.invoked");
    install_fake_cargo(&fake_bin, &app, &cargo_invoked);
    let output = run_native_dogfood_script(&temp, &fake_bin, &app)
        .env("RUI_NATIVE_DOGFOOD_PROFILE", &upper)
        .env("RUI_NATIVE_DOGFOOD_RENDERER_PROFILE", &lower)
        .output()
        .expect("native dogfood script should run");

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("distinct filesystem entries"));
    assert!(
        !cargo_invoked.exists(),
        "build must not start for aliased artifacts"
    );
}

#[test]
fn canonicalizes_leading_dash_artifact_paths_without_option_injection() {
    let temp = DogfoodTestDir::new("leading-dash");
    let fake_bin = temp.path().join("bin");
    fs::create_dir(&fake_bin).expect("fake bin directory should be created");
    let app = temp.path().join("native_dogfood");
    write_executable(
        &app,
        r#"#!/usr/bin/env bash
set -euo pipefail
printf '{"status":"passed","typed_text":"%s","script_requires_minimize_reopen":true}\n' \
  "$RUI_NATIVE_DOGFOOD_TEXT" >"$RUI_NATIVE_DOGFOOD_PROFILE"
printf '{"schema":"rui.renderer.profile.v1","status":"passed"}\n'
"#,
    );
    install_fake_cargo(&fake_bin, &app, &temp.path().join("cargo.invoked"));
    let unrelated = temp.path().join("unrelated.txt");
    fs::write(&unrelated, "preserve").expect("unrelated artifact should be written");

    let output = run_native_dogfood_script(&temp, &fake_bin, &app)
        .current_dir(temp.path())
        .env("RUI_NATIVE_DOGFOOD_PROFILE", "-r")
        .env("RUI_NATIVE_DOGFOOD_RENDERER_PROFILE", "renderer.jsonl")
        .env("RUI_NATIVE_DOGFOOD_LOG", "dogfood.log")
        .output()
        .expect("native dogfood script should run");

    assert!(output.status.success(), "stderr={}", stderr(&output));
    assert_eq!(
        fs::read_to_string(temp.path().join("-r")).expect("leading-dash profile should be written"),
        "{\"status\":\"passed\",\"typed_text\":\"rui-native-dogfood\",\"script_requires_minimize_reopen\":true}\n"
    );
    assert_eq!(
        fs::read_to_string(&unrelated).expect("unrelated artifact should remain"),
        "preserve"
    );
}

#[test]
fn canonicalization_failure_stops_before_removing_other_artifacts() {
    let temp = DogfoodTestDir::new("canonical-failure");
    let fake_bin = temp.path().join("bin");
    fs::create_dir(&fake_bin).expect("fake bin directory should be created");
    let app = temp.path().join("native_dogfood");
    write_executable(&app, "#!/usr/bin/env bash\nexit 0\n");
    let cargo_invoked = temp.path().join("cargo.invoked");
    install_fake_cargo(&fake_bin, &app, &cargo_invoked);
    let blocked_parent = temp.path().join("not-a-directory");
    let renderer = temp.path().join("renderer.jsonl");
    let log = temp.path().join("dogfood.log");
    fs::write(&blocked_parent, "file").expect("blocking file should be written");
    fs::write(&renderer, "renderer-preserve").expect("renderer sentinel should be written");
    fs::write(&log, "log-preserve").expect("log sentinel should be written");

    let output = run_native_dogfood_script(&temp, &fake_bin, &app)
        .env(
            "RUI_NATIVE_DOGFOOD_PROFILE",
            blocked_parent.join("profile.json"),
        )
        .env("RUI_NATIVE_DOGFOOD_RENDERER_PROFILE", &renderer)
        .env("RUI_NATIVE_DOGFOOD_LOG", &log)
        .output()
        .expect("native dogfood script should run");

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("artifact paths could not be canonicalized"));
    assert_eq!(
        fs::read_to_string(&renderer).expect("renderer sentinel should remain"),
        "renderer-preserve"
    );
    assert_eq!(
        fs::read_to_string(&log).expect("log sentinel should remain"),
        "log-preserve"
    );
    assert!(
        !cargo_invoked.exists(),
        "build must not start after path failure"
    );
}

struct DogfoodTestDir(PathBuf);

impl DogfoodTestDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "rui-dogfood-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should follow the Unix epoch")
                .as_nanos()
        ));
        fs::create_dir(&path).expect("dogfood test directory should be created");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for DogfoodTestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("test executable should be written");
    let mut permissions = fs::metadata(path)
        .expect("test executable metadata should exist")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("test executable should become executable");
}

fn install_fake_cargo(fake_bin: &Path, app: &Path, invoked: &Path) {
    let cargo = fake_bin.join("cargo");
    write_executable(
        &cargo,
        r#"#!/usr/bin/env bash
set -euo pipefail
printf invoked >"$FAKE_CARGO_INVOKED"
if [[ "$*" == *"build --example native_dogfood"* ]]; then
  printf '{"executable":"%s"}\n' "$FAKE_APP_PATH"
  exit 0
fi
if [[ "$*" == *"validate_renderer_profile"* ]]; then
  exit 0
fi
exit 9
"#,
    );
    assert!(app.is_absolute());
    assert!(invoked.is_absolute());
}

fn run_native_dogfood_script(temp: &DogfoodTestDir, fake_bin: &Path, app: &Path) -> Command {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut command = Command::new(manifest_dir.join("scripts/native_dogfood_macos.sh"));
    let path = format!(
        "{}:{}",
        fake_bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    command
        .current_dir(manifest_dir)
        .env("PATH", path)
        .env("FAKE_APP_PATH", app)
        .env("FAKE_CARGO_INVOKED", temp.path().join("cargo.invoked"))
        .env(
            "RUI_NATIVE_DOGFOOD_PROFILE",
            temp.path().join("profile.json"),
        )
        .env(
            "RUI_NATIVE_DOGFOOD_RENDERER_PROFILE",
            temp.path().join("renderer.jsonl"),
        )
        .env("RUI_NATIVE_DOGFOOD_LOG", temp.path().join("dogfood.log"));
    command
}

fn output_with_deadline(command: &mut Command, deadline: Duration) -> Result<Output, &'static str> {
    let mut child = command
        .process_group(0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "failed to spawn script")?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|_| "failed to read output");
            }
            Ok(None) if started.elapsed() < deadline => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Ok(None) => {
                let process_group = format!("-{}", child.id());
                let _ = Command::new("kill")
                    .args(["-KILL", process_group.as_str()])
                    .status();
                let _ = child.wait();
                return Err("deadline exceeded");
            }
            Err(_) => return Err("failed to poll script"),
        }
    }
}

fn assert_process_gone(pid_file: &Path) {
    let pid = fs::read_to_string(pid_file).expect("application pid should be recorded");
    assert!(
        !Command::new("kill")
            .args(["-0", pid.trim()])
            .output()
            .expect("kill probe should run")
            .status
            .success(),
        "timed-out application process should no longer exist"
    );
}

fn assert_pid_recorded(pid_file: &Path, output: &Output, temp_path: &Path) {
    let log = fs::read_to_string(temp_path.join("dogfood.log")).unwrap_or_default();
    assert!(
        pid_file.exists(),
        "application did not record its pid; stderr={} log={log}",
        stderr(output)
    );
}

fn force_kill_recorded_process(pid_file: &Path) {
    if let Ok(pid) = fs::read_to_string(pid_file) {
        let _ = Command::new("kill").args(["-KILL", pid.trim()]).status();
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
