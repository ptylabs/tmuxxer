#!/usr/bin/env python3
"""Compare picker preparation using temporary folders and stub external tools."""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binaries", nargs="+", type=Path)
    parser.add_argument("--runs", type=int, default=20)
    parser.add_argument("--directories", type=int, default=10000)
    args = parser.parse_args()
    if args.runs < 1 or args.directories < 1:
        parser.error("runs and directories must be positive")
    with tempfile.TemporaryDirectory(prefix="tmuxxer-bench-") as temporary:
        root = Path(temporary)
        projects = root / "projects"
        projects.mkdir()
        for index in range(args.directories):
            (projects / f"project-{index:05}").mkdir()
        bin_dir = root / "bin"
        bin_dir.mkdir()
        for name, script in {
            "tmux": 'if [ "$1" = "-V" ]; then exit 0; fi\nsleep 0.02\nprintf "work\\n"\n',
            "docker": 'sleep 0.04\nprintf "abc\\tweb\\tnginx\\n"\n',
            "fzf": 'if [ "$1" = "--version" ]; then exit 0; fi\ncat >/dev/null\nexit 130\n',
        }.items():
            path = bin_dir / name
            path.write_text("#!/bin/sh\n" + script)
            path.chmod(0o755)
        config = root / "tmuxxer" / "config"
        config.parent.mkdir()
        env = dict(os.environ, PATH=f"{bin_dir}:/usr/bin:/bin", XDG_CONFIG_HOME=str(root),
                   XDG_CACHE_HOME=str(root / "cache"))
        for scenario, external in [("directories", False), ("mixed", True)]:
            enabled = str(external).lower()
            config.write_text(
                f"version = 2\n[sources]\nsessions = {enabled}\ndocker = {enabled}\n"
                "directories = true\n[updates]\nauto_check = false\n"
                f"[[search.roots]]\npath = {json.dumps(str(projects))}\ndepth = 1\n"
            )
            for binary in args.binaries:
                binary = binary.resolve()
                samples = []
                for run in range(args.runs + 3):
                    start = time.perf_counter()
                    subprocess.run([str(binary)], env=env, check=True,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
                    elapsed = (time.perf_counter() - start) * 1000
                    if run >= 3:
                        samples.append(elapsed)
                print(f"{scenario}: {binary} | {binary.stat().st_size} bytes | "
                      f"median {statistics.median(samples):.2f} ms | "
                      f"min {min(samples):.2f} ms")


if __name__ == "__main__":
    main()
