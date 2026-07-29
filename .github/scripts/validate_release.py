#!/usr/bin/env python3
"""Validate Wuddle release metadata and extract exact release notes."""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path


SEMVER = re.compile(
    r"""
    (0|[1-9]\d*)\.
    (0|[1-9]\d*)\.
    (0|[1-9]\d*)
    (?:-
        (?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)
        (?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*
    )?
    (?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?
    """,
    re.VERBOSE,
)


class ReleaseValidationError(RuntimeError):
    """A release invariant was not satisfied."""


def parse_tag(tag: str) -> str:
    if not tag.startswith("v") or not SEMVER.fullmatch(tag[1:]):
        raise ReleaseValidationError(
            f"release tag {tag!r} must be an exact v-prefixed SemVer"
        )
    return tag[1:]


def load_toml(path: Path) -> dict:
    try:
        with path.open("rb") as handle:
            return tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ReleaseValidationError(f"could not read {path}: {error}") from error


def exact_changelog_section(root: Path, version: str) -> str:
    path = root / "CHANGELOG.md"
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        raise ReleaseValidationError(f"could not read {path}: {error}") from error

    heading = re.compile(rf"^## v{re.escape(version)}\s*$", re.MULTILINE)
    matches = list(heading.finditer(text))
    if len(matches) != 1:
        raise ReleaseValidationError(
            f"CHANGELOG.md must contain exactly one '## v{version}' heading"
        )

    start = matches[0].end()
    next_heading = re.search(r"^## v", text[start:], re.MULTILINE)
    end = start + next_heading.start() if next_heading else len(text)
    notes = text[start:end].strip()
    if not notes:
        raise ReleaseValidationError(
            f"CHANGELOG.md section for v{version} must not be empty"
        )
    return notes


def release_notes(root: Path, version: str) -> str:
    path = root / ".github" / "release-notes" / f"v{version}.md"
    if not path.exists():
        return exact_changelog_section(root, version)
    try:
        notes = path.read_text(encoding="utf-8").strip()
    except OSError as error:
        raise ReleaseValidationError(f"could not read {path}: {error}") from error
    if not notes:
        raise ReleaseValidationError(f"{path} must not be empty")
    return notes


def validate_release(root: Path, tag: str) -> str:
    version = parse_tag(tag)

    manifest = load_toml(root / "wuddle-iced" / "Cargo.toml")
    manifest_version = manifest.get("package", {}).get("version")
    if manifest_version != version:
        raise ReleaseValidationError(
            f"tag {tag} does not match wuddle-iced version {manifest_version!r}"
        )

    lockfile = load_toml(root / "wuddle-iced" / "Cargo.lock")
    locked_versions = {
        package.get("version")
        for package in lockfile.get("package", [])
        if package.get("name") == "wuddle-iced"
    }
    if locked_versions != {version}:
        raise ReleaseValidationError(
            "wuddle-iced/Cargo.lock does not contain the exact release version "
            f"{version!r}"
        )

    exact_changelog_section(root, version)
    release_notes(root, version)

    readme_path = root / "README.md"
    try:
        readme = readme_path.read_text(encoding="utf-8")
    except OSError as error:
        raise ReleaseValidationError(
            f"could not read {readme_path}: {error}"
        ) from error
    readme_heading = re.compile(
        rf"^### What's New in v{re.escape(version)}\s*$", re.MULTILINE
    )
    if len(readme_heading.findall(readme)) != 1:
        raise ReleaseValidationError(
            f"README.md must contain exactly one \"What's New in v{version}\" heading"
        )

    return version


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("tag", help="v-prefixed release tag")
    parser.add_argument(
        "--root",
        type=Path,
        default=Path.cwd(),
        help="repository root (defaults to the current directory)",
    )
    parser.add_argument(
        "--notes",
        action="store_true",
        help="print only the exact validated changelog section",
    )
    args = parser.parse_args(argv)

    try:
        version = validate_release(args.root, args.tag)
        if args.notes:
            print(release_notes(args.root, version))
        else:
            print(f"Release metadata validated for {args.tag}.")
    except ReleaseValidationError as error:
        print(f"Release validation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
