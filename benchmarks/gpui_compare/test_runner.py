#!/usr/bin/env python3
"""Exercise runner artifacts with tiny fake tools; no Cargo build or GPU work."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


with tempfile.TemporaryDirectory(prefix="rui-gpu-runner-") as temporary:
    root = Path(temporary) / "fixture with spaces"
    bench = root / "benchmarks/gpui_compare"
    bench.mkdir(parents=True)
    shutil.copyfile(Path(__file__).with_name("run.sh"), bench / "run.sh")
    for name in ("fixtures.rs", "gpu_time.rs"):
        (bench / name).write_text("// fixture source\n")
    zed = root / "zed checkout"
    (zed / ".git").mkdir(parents=True)
    platform = zed / "crates/gpui_platform"
    (platform / "benches").mkdir(parents=True)
    (platform / "Cargo.toml").write_text('name = "rui_compare"\nimage.workspace = true\n')
    commands = root / "fake-tools"
    commands.mkdir()

    def executable(path, contents):
        path.write_text("#!/usr/bin/env bash\nset -eu\n" + contents)
        path.chmod(0o755)

    executable(commands / "git", '''
case "$3" in
  rev-parse) echo 784374c6794ace885137bc4806427a5f47cc3f06 ;;
  status) echo ' M tracked-source.rs' ;;
  diff) echo 'fixture source diff' ;;
  *) exit 91 ;;
esac
''')
    executable(commands / "cargo", '''
if [ "$1" = --version ]; then echo 'cargo fixture'; exit; fi
if [ "${FAIL_BUILD:-}" = 1 ]; then echo 'build diagnostic' >&2; exit 8; fi
case "$PWD" in *'zed checkout') side=gpui ;; *) side=rui ;; esac
if [ "$PATH_MODE" = absolute ]; then binary="$PWD/$side-bench"; else binary="$side-bench"; fi
printf 'Executable bench (%s)\\n' "$binary" >&2
''')
    executable(commands / "rustc", "echo 'rustc fixture'\n")
    executable(commands / "sysctl", "echo 'machdep.cpu.brand_string: fixture CPU'\n")
    executable(commands / "sw_vers", "echo 'ProductVersion: fixture'\n")
    executable(root / "rui-bench", '''
[ "$1" != --list ] || exit 0
echo 'rui diagnostic' >&2
echo "rui $1 instructions_m=1 ms_p50=1 gpu_ms_p50=0.2"
''')
    executable(zed / "gpui-bench", '''
[ "$1" != --list ] || exit 0
echo 'gpui stdout marker'
echo 'gpui diagnostic' >&2
[ "${FAIL_GPUI:-}" != 1 ] || exit 7
echo "gpui $3 draw_calls=2 gpu_ms_p50=0.1" >&2
echo 'time: [1 M instructions 2 M instructions 3 M instructions]' >&2
echo '  wall time per iteration: median 2 ms' >&2
''')
    env = dict(os.environ, PATH=f"{commands}:{os.environ['PATH']}", REPEATS="2", CASES="list_virtual_1k")
    output = root / "target/gpui_compare"

    def run(**changes):
        before = set(output.glob("run.*")) if output.exists() else set()
        result = subprocess.run(["bash", str(bench / "run.sh"), "zed checkout"],
                                cwd=root, env=dict(env, **changes), text=True, capture_output=True)
        created = set(output.glob("run.*")) - before
        assert len(created) == 1, (created, result.stderr)
        return result, created.pop()

    for mode in ("absolute", "relative"):
        result, artifact = run(PATH_MODE=mode)
        assert result.returncode == 0, result.stderr
        assert "not equivalent" in result.stdout
        assert "verdict by GPU" not in result.stdout
        metadata = (artifact / "metadata.txt").read_text()
        assert "rui_revision=784374c" in metadata and "zed_revision=784374c" in metadata
        assert "repeats=2" in metadata and "cases=list_virtual_1k" in metadata
        assert " M tracked-source.rs" in metadata and "ProductVersion: fixture" in metadata
        reported = next(line.split("=", 1)[1] for line in metadata.splitlines() if line.startswith("gpui_executable="))
        assert Path(reported).resolve() == (zed / "gpui-bench").resolve()
        assert (artifact / "rui-source.patch").read_text() == "fixture source diff\n"
        assert (artifact / "fixtures.rs").read_text() == "// fixture source\n"
        order = (artifact / "order.txt").read_text().splitlines()
        assert [line.rsplit("=", 1)[1] for line in order] == ["rui", "gpui", "gpui", "rui"]
        for round_number in (1, 2):
            prefix = artifact / f"round-{round_number}-list_virtual_1k-gpui"
            assert Path(str(prefix) + ".stdout").read_text() == "gpui stdout marker\n"
            assert "gpui diagnostic" in Path(str(prefix) + ".stderr").read_text()
        assert "gpui list_virtual_1k" in (artifact / "results.txt").read_text()
    assert len(list(output.glob("run.*"))) == 2, "later run overwrote earlier evidence"
    result, artifact = run(PATH_MODE="absolute", FAIL_GPUI="1")
    assert result.returncode == 7
    assert "gpui diagnostic" in (artifact / "round-1-list_virtual_1k-gpui.stderr").read_text()
    assert "| case |" not in result.stdout, "failed benchmark emitted a completed table"
    result, artifact = run(PATH_MODE="absolute", FAIL_BUILD="1")
    assert result.returncode != 0
    assert "build diagnostic" in (artifact / "build-rui.log").read_text()
    print("Verified absolute/relative executable paths, separate runs, metadata, AB/BA, raw output, and failure evidence.")
