from util.v3_core.legacy_fortran import LegacyFortranRun, build_legacy_command


def test_build_legacy_command_points_at_mkgrd_and_namelist(tmp_path):
    executable = tmp_path / "mkgrd.x"
    executable.write_text("#!/bin/sh\nexit 0\n")
    namelist = tmp_path / "case.nml"
    namelist.write_text("&mkgrd\n/\n")

    command = build_legacy_command(executable, namelist)

    assert command == [str(executable), str(namelist)]


def test_legacy_run_dry_run_does_not_execute(tmp_path):
    executable = tmp_path / "mkgrd.x"
    namelist = tmp_path / "case.nml"
    run = LegacyFortranRun(executable=executable, namelist=namelist, workdir=tmp_path)

    result = run.run(dry_run=True)

    assert result.returncode == 0
    assert result.executed is False
    assert result.command == [str(executable), str(namelist)]
