import errno
import fcntl
import json
import os
import pty
import re
import select
import shlex
import shutil
import signal
import struct
import subprocess
import tempfile
import termios
import time

binary = os.path.abspath("target/debug/sagebrush")
trials = {"cli_initial": [], "tui_initial": [], "refresh": [], "resize": []}


class Screen:
    def __init__(self, rows=30, cols=120):
        self.resize(rows, cols)
        self.state = ""
        self.csi = ""

    def resize(self, rows, cols):
        self.rows, self.cols = rows, cols
        self.cells = [[" "] * cols for _ in range(rows)]
        self.y = self.x = 0

    def feed(self, data):
        for ch in data.decode("utf-8", "replace"):
            if self.state == "esc":
                self.state = "csi" if ch == "[" else ""
                self.csi = ""
            elif self.state == "csi":
                if "@" <= ch <= "~":
                    self.control(ch, self.csi)
                    self.state = ""
                else:
                    self.csi += ch
            elif ch == "\x1b":
                self.state = "esc"
            elif ch == "\r":
                self.x = 0
            elif ch == "\n":
                self.y = min(self.y + 1, self.rows - 1)
            elif ch >= " " and ch != "\x7f":
                if 0 <= self.y < self.rows and 0 <= self.x < self.cols:
                    self.cells[self.y][self.x] = ch
                self.x += 1

    def control(self, final, args):
        if args.startswith("?"):
            return
        values = [int(x) if x.isdigit() else 0 for x in args.split(";")]
        if final in "Hf":
            self.y = (values[0] or 1) - 1
            self.x = (values[1] or 1) - 1 if len(values) > 1 else 0
        elif final == "G":
            self.x = (values[0] or 1) - 1
        elif final == "d":
            self.y = (values[0] or 1) - 1
        elif final == "J" and values[0] == 2:
            self.cells = [[" "] * self.cols for _ in range(self.rows)]
        elif final == "K" and 0 <= self.y < self.rows:
            for x in range(max(0, self.x), self.cols):
                self.cells[self.y][x] = " "

    def text(self):
        return "\n".join("".join(row) for row in self.cells)


class Tui:
    def __init__(self, env):
        self.master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 120, 0, 0))
        self.screen = Screen()
        self.start = time.monotonic()
        self.process = subprocess.Popen(
            [binary, "--no-color"], stdin=slave, stdout=slave, stderr=slave,
            env=env, start_new_session=True,
        )
        os.close(slave)
        os.set_blocking(self.master, False)

    def wait(self, predicate, timeout):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if predicate(self.screen.text()):
                return True
            if self.process.poll() is not None:
                raise RuntimeError(f"TUI exited {self.process.returncode}: {self.screen.text()}")
            ready, _, _ = select.select([self.master], [], [], max(0, min(.05, deadline - time.monotonic())))
            if ready:
                try:
                    self.screen.feed(os.read(self.master, 65536))
                except OSError as error:
                    if error.errno != errno.EAGAIN:
                        raise
        return predicate(self.screen.text())

    def resize(self, cols, rows):
        self.screen.resize(rows, cols)
        fcntl.ioctl(self.master, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
        os.kill(self.process.pid, signal.SIGWINCH)

    def stop(self):
        if self.process.poll() is None:
            os.write(self.master, b"q")
            try:
                self.process.wait(timeout=8)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
                raise RuntimeError("TUI failed to quit")
        status = self.process.returncode
        os.close(self.master)
        if status != 0:
            raise RuntimeError(f"TUI quit with status {status}")


with tempfile.TemporaryDirectory(prefix="sagebrush-measure-") as directory:
    env = os.environ.copy()
    env.update(TMUX_TMPDIR=directory, TERM="xterm", NO_COLOR="1")
    env.pop("TMUX", None)
    env.pop("TMUX_PANE", None)

    real_tmux = shutil.which("tmux")
    if not real_tmux:
        raise RuntimeError("tmux is required")
    wrapper_dir = os.path.join(directory, "bin")
    os.mkdir(wrapper_dir)
    wrapper = os.path.join(wrapper_dir, "tmux")
    marker = os.path.join(directory, "block")
    pid_file = os.path.join(directory, "blocked-pid")
    with open(wrapper, "w", encoding="utf-8") as file:
        file.write(
            "#!/bin/sh\n"
            "if [ -e \"$SAGEBRUSH_MEASURE_BLOCK\" ]; then\n"
            "  echo $$ > \"$SAGEBRUSH_MEASURE_PID\"\n"
            "  exec /bin/sleep 30\n"
            "fi\n"
            f"exec {shlex.quote(real_tmux)} \"$@\"\n"
        )
    os.chmod(wrapper, 0o700)
    env["PATH"] = wrapper_dir + os.pathsep + env["PATH"]
    env["SAGEBRUSH_MEASURE_BLOCK"] = marker
    env["SAGEBRUSH_MEASURE_PID"] = pid_file

    def tmux(*args):
        return subprocess.run([real_tmux, *args], env=env, check=True, capture_output=True, text=True).stdout

    def state():
        return tmux("list-sessions", "-F", "#{session_id}\t#{session_name}\t#{session_windows}").splitlines()

    running = False
    try:
        for i in range(100):
            tmux("new-session", "-d", "-s", f"bench{i:03}", "-x", "120", "-y", "30", "exec sleep 1800")
            running = True
        baseline = state()
        for _ in range(20):
            started = time.monotonic()
            result = subprocess.run([binary, "list", "--json"], env=env, capture_output=True, text=True, timeout=8)
            elapsed = time.monotonic() - started
            parsed = json.loads(result.stdout) if result.returncode == 0 else None
            ok = parsed is not None and len(parsed) == 100 and {item["name"] for item in parsed} == {f"bench{i:03}" for i in range(100)}
            trials["cli_initial"].append({"seconds": round(elapsed, 6), "visible": ok, "status": result.returncode})
        for _ in range(20):
            ui = Tui(env)
            try:
                visible = ui.wait(lambda s: "bench000" in s and "bench022" in s and "WINDOWS" in s, 8)
                trials["tui_initial"].append({"seconds": round(time.monotonic() - ui.start, 6), "visible": visible})
            finally:
                ui.stop()
        trials["initial_runs_read_only"] = state() == baseline
        ui = Tui(env)
        try:
            if not ui.wait(lambda s: "bench000" in s and "bench022" in s and "WINDOWS" in s, 8):
                raise RuntimeError("TUI initial data not visible")
            for i in range(20):
                started = time.monotonic()
                name = f"probe{i:02}"
                tmux("rename-session", "-t", "$0", name)
                tmux("new-window", "-d", "-t", "$0:", "-n", f"extra{i:02}", "exec sleep 1800")
                tmux("kill-session", "-t", f"bench{i + 1:03}")
                tmux("new-session", "-d", "-s", f"tail{i:02}", "-x", "120", "-y", "30", "exec sleep 1800")
                count = i + 2
                newly_visible = f"bench{i + 23:03}"
                visible = ui.wait(
                    lambda s: newly_visible in s and f"bench{i + 1:03}" not in s and any(name in line and re.search(rf"\b{count}\b", line.split(name, 1)[1]) for line in s.splitlines()),
                    8,
                )
                trials["refresh"].append({"seconds": round(time.monotonic() - started, 6), "visible": visible})
            state_before_resizes = state()
            open(marker, "w", encoding="utf-8").close()
            deadline = time.monotonic() + 8
            while not os.path.exists(pid_file) or not open(pid_file, encoding="utf-8").read().strip():
                if time.monotonic() >= deadline:
                    raise RuntimeError("refresh did not block")
                time.sleep(.01)
            blocked_pid = int(open(pid_file, encoding="utf-8").read())
            os.kill(blocked_pid, 0)
            for i in range(20):
                cols, rows = [(39, 10), (40, 10), (80, 24), (120, 30)][i % 4]
                started = time.monotonic()
                ui.resize(cols, rows)
                visible = ui.wait(
                    lambda s: "Terminal too small" in s if cols < 40 else "probe19" in s and "Sagebrush" in s,
                    5,
                )
                trials["resize"].append({"seconds": round(time.monotonic() - started, 6), "visible": visible, "size": f"{cols}x{rows}"})
            os.kill(blocked_pid, 0)
            trials["resizes_during_blocked_refresh"] = True
        finally:
            ui.stop()
        try:
            os.kill(blocked_pid, 0)
        except ProcessLookupError:
            trials["blocked_query_reaped"] = True
        else:
            raise RuntimeError("blocked query survived TUI exit")
        trials["resize_run_read_only"] = state() == state_before_resizes
    finally:
        if running:
            tmux("kill-server")

print(json.dumps(trials, indent=2))
