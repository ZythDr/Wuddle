from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from validate_release import ReleaseValidationError, release_notes, validate_release


class ValidateReleaseTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name)
        (self.root / "wuddle-iced").mkdir()
        self.write_valid_fixture()

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def write_valid_fixture(self) -> None:
        (self.root / "wuddle-iced" / "Cargo.toml").write_text(
            '[package]\nname = "wuddle-iced"\nversion = "3.7.0-beta.7"\n',
            encoding="utf-8",
        )
        (self.root / "wuddle-iced" / "Cargo.lock").write_text(
            'version = 4\n\n[[package]]\nname = "wuddle-iced"\n'
            'version = "3.7.0-beta.7"\n',
            encoding="utf-8",
        )
        (self.root / "CHANGELOG.md").write_text(
            "# Changelog\n\n## v3.7.0-beta.7\n\n- Exact beta notes.\n\n"
            "## v3.7.0\n\n- Stable notes.\n",
            encoding="utf-8",
        )
        (self.root / "README.md").write_text(
            "# Wuddle\n\n### What's New in v3.7.0-beta.7\n",
            encoding="utf-8",
        )

    def test_accepts_an_exact_prerelease(self) -> None:
        self.assertEqual(
            validate_release(self.root, "v3.7.0-beta.7"),
            "3.7.0-beta.7",
        )

    def test_uses_condensed_release_notes_when_present(self) -> None:
        notes_dir = self.root / ".github" / "release-notes"
        notes_dir.mkdir(parents=True)
        (notes_dir / "v3.7.0-beta.7.md").write_text(
            "Condensed beta notes.\n",
            encoding="utf-8",
        )
        self.assertEqual(
            release_notes(self.root, "3.7.0-beta.7"),
            "Condensed beta notes.",
        )

    def test_falls_back_to_the_exact_changelog_without_condensed_notes(self) -> None:
        self.assertEqual(
            release_notes(self.root, "3.7.0-beta.7"),
            "- Exact beta notes.",
        )

    def test_rejects_empty_condensed_release_notes(self) -> None:
        notes_dir = self.root / ".github" / "release-notes"
        notes_dir.mkdir(parents=True)
        (notes_dir / "v3.7.0-beta.7.md").write_text("\n", encoding="utf-8")
        with self.assertRaisesRegex(ReleaseValidationError, "must not be empty"):
            validate_release(self.root, "v3.7.0-beta.7")

    def test_rejects_a_tag_manifest_mismatch(self) -> None:
        with self.assertRaisesRegex(ReleaseValidationError, "does not match"):
            validate_release(self.root, "v3.7.0-beta.8")

    def test_rejects_a_missing_exact_prerelease_changelog(self) -> None:
        (self.root / "CHANGELOG.md").write_text(
            "# Changelog\n\n## v3.7.0\n\n- Stable notes.\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(ReleaseValidationError, "exactly one"):
            validate_release(self.root, "v3.7.0-beta.7")

    def test_rejects_an_unlocked_release_version(self) -> None:
        (self.root / "wuddle-iced" / "Cargo.lock").write_text(
            'version = 4\n\n[[package]]\nname = "wuddle-iced"\n'
            'version = "3.7.0-beta.6"\n',
            encoding="utf-8",
        )
        with self.assertRaisesRegex(ReleaseValidationError, "Cargo.lock"):
            validate_release(self.root, "v3.7.0-beta.7")

    def test_rejects_malformed_semver(self) -> None:
        with self.assertRaisesRegex(ReleaseValidationError, "SemVer"):
            validate_release(self.root, "v3.07.0-beta.7")

    def test_requires_the_current_readme_heading(self) -> None:
        (self.root / "README.md").write_text("# Wuddle\n", encoding="utf-8")
        with self.assertRaisesRegex(ReleaseValidationError, "README.md"):
            validate_release(self.root, "v3.7.0-beta.7")


if __name__ == "__main__":
    unittest.main()
