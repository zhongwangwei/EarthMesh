from __future__ import annotations

import subprocess
from dataclasses import dataclass
from pathlib import Path


def build_legacy_command(executable: str | Path, namelist: str | Path) -> list[str]:
    return [str(Path(executable)), str(Path(namelist))]


@dataclass(frozen=True)
class LegacyFortranResult:
    command: list[str]
    returncode: int
    stdout: str
    stderr: str
    executed: bool


@dataclass(frozen=True)
class LegacyFortranRun:
    executable: Path
    namelist: Path
    workdir: Path

    def run(self, *, dry_run: bool = True) -> LegacyFortranResult:
        command = build_legacy_command(self.executable, self.namelist)
        if dry_run:
            return LegacyFortranResult(command=command, returncode=0, stdout="", stderr="", executed=False)
        completed = subprocess.run(command, cwd=self.workdir, check=False, capture_output=True, text=True)
        return LegacyFortranResult(
            command=command,
            returncode=completed.returncode,
            stdout=completed.stdout,
            stderr=completed.stderr,
            executed=True,
        )
