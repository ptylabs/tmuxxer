#!/usr/bin/env python3
"""Regression: tmux clients need a real PTY, including when ZLE disconnects stdin.

Run after `cargo test --locked` on Linux or macOS.
"""
import os
from pathlib import Path
import pty
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
rlib = max((root / 'target/debug/deps').glob('libtmuxxer-*.rlib'), key=lambda p: p.stat().st_mtime)
with tempfile.TemporaryDirectory(prefix='tmuxxer-tty-test-') as directory:
    tmp = Path(directory)
    mock = tmp / 'tmux'
    mock.write_text('''#!/bin/sh
case "$*" in *"-d"*) exit 0;; esac
terminal=$(tty) || exit 1
[ "$terminal" != /dev/tty ]
''')
    mock.chmod(0o755)
    source = tmp / 'probe.rs'
    source.write_text('''use tmuxxer::tmux::{SystemTmux, TmuxCommand};
fn main() {
    match std::env::args().nth(1).unwrap().as_str() {
        "attach" => SystemTmux.attach("probe").unwrap(),
        "new" => SystemTmux.new_session("probe", std::path::Path::new("/tmp"), false).unwrap(),
        "command" => SystemTmux.new_session_with_command("probe", "sh", false).unwrap(),
        _ => SystemTmux.new_session("probe", std::path::Path::new("/tmp"), true).unwrap(),
    }
}
''')
    probe = tmp / 'probe'
    subprocess.run(['rustc', '--edition=2024', str(source), '--extern', f'tmuxxer={rlib}',
                    '-L', f'dependency={rlib.parent}', '-o', str(probe)], check=True)
    env = dict(os.environ, PATH=directory + os.pathsep + os.environ['PATH'])
    for stdin in ('/dev/null', '/dev/tty'):
        for mode in ('attach', 'new', 'command'):
            pid, master = pty.fork()
            if pid == 0:
                fd = os.open(stdin, os.O_RDONLY)
                os.dup2(fd, 0)
                os.close(fd)
                os.execve(str(probe), [str(probe), mode], env)
            _, status = os.waitpid(pid, 0)
            os.close(master)
            assert os.waitstatus_to_exitcode(status) == 0, (stdin, mode)
            print(f'PASS {mode}: stdin={stdin}, client receives real PTY')
    subprocess.run([str(probe), 'detached'], env=env, stdin=subprocess.DEVNULL,
                   start_new_session=True, check=True)
    print('PASS detached: no terminal required')
