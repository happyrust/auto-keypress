import importlib.util
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
MODULE_PATH = REPO_ROOT / ".github" / "scripts" / "release_meta.py"


spec = importlib.util.spec_from_file_location("release_meta", MODULE_PATH)
release_meta = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = release_meta
spec.loader.exec_module(release_meta)


class ReleaseMetaTests(unittest.TestCase):
    def write_temp_file(self, directory: Path, name: str, content: str) -> Path:
        path = directory / name
        path.write_text(textwrap.dedent(content).lstrip(), encoding="utf-8")
        return path

    def test_build_release_meta_marks_missing_tag_for_release(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp_path = Path(tmpdir)
            cargo_path = self.write_temp_file(
                tmp_path,
                "Cargo.toml",
                '''
                [package]
                name = "auto-keypress"
                version = "0.2.1"
                edition = "2021"
                ''',
            )
            changelog_path = self.write_temp_file(
                tmp_path,
                "CHANGELOG.md",
                '''
                # 更新日志

                ## v0.2.1 (2026-04-16)

                - 自动创建 Release

                ## v0.2.0 (2026-04-15)

                - 旧版本
                ''',
            )

            meta = release_meta.build_release_meta(
                cargo_path=cargo_path,
                changelog_path=changelog_path,
                existing_tags=[],
            )

            self.assertEqual(meta.version, "0.2.1")
            self.assertEqual(meta.tag_name, "v0.2.1")
            self.assertFalse(meta.tag_exists)
            self.assertTrue(meta.should_release)
            self.assertEqual(
                meta.release_body,
                "## v0.2.1 (2026-04-16)\n\n- 自动创建 Release",
            )

    def test_build_release_meta_skips_release_when_tag_exists(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp_path = Path(tmpdir)
            cargo_path = self.write_temp_file(
                tmp_path,
                "Cargo.toml",
                '''
                [package]
                name = "auto-keypress"
                version = "0.2.1"
                edition = "2021"
                ''',
            )
            changelog_path = self.write_temp_file(
                tmp_path,
                "CHANGELOG.md",
                '''
                # 更新日志

                ## v0.2.1 (2026-04-16)

                - 自动创建 Release
                ''',
            )

            meta = release_meta.build_release_meta(
                cargo_path=cargo_path,
                changelog_path=changelog_path,
                existing_tags=["v0.2.1", "v0.2.0"],
            )

            self.assertTrue(meta.tag_exists)
            self.assertFalse(meta.should_release)

    def test_build_release_meta_rejects_mismatched_changelog_version(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp_path = Path(tmpdir)
            cargo_path = self.write_temp_file(
                tmp_path,
                "Cargo.toml",
                '''
                [package]
                name = "auto-keypress"
                version = "0.2.2"
                edition = "2021"
                ''',
            )
            changelog_path = self.write_temp_file(
                tmp_path,
                "CHANGELOG.md",
                '''
                # 更新日志

                ## v0.2.1 (2026-04-16)

                - 自动创建 Release
                ''',
            )

            with self.assertRaisesRegex(ValueError, "CHANGELOG"):
                release_meta.build_release_meta(
                    cargo_path=cargo_path,
                    changelog_path=changelog_path,
                    existing_tags=[],
                )

    def test_write_github_output_writes_multiline_release_body(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp_path = Path(tmpdir)
            output_path = tmp_path / "github_output.txt"
            body_path = tmp_path / "release_body.md"
            meta = release_meta.ReleaseMeta(
                version="0.2.1",
                tag_name="v0.2.1",
                tag_exists=False,
                should_release=True,
                release_body="## v0.2.1 (2026-04-16)\n\n- 自动创建 Release",
            )

            release_meta.write_release_body(body_path, meta.release_body)
            release_meta.write_github_output(output_path, meta, body_path)

            self.assertEqual(body_path.read_text(encoding="utf-8"), meta.release_body)
            output_text = output_path.read_text(encoding="utf-8")
            self.assertIn("version=0.2.1", output_text)
            self.assertIn("tag_name=v0.2.1", output_text)
            self.assertIn("tag_exists=false", output_text)
            self.assertIn("should_release=true", output_text)
            self.assertIn(f"release_body_path={body_path}", output_text)


if __name__ == "__main__":
    unittest.main()
