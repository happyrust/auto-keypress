#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


PACKAGE_VERSION_RE = re.compile(r'^version\s*=\s*"([^"]+)"\s*$')
CHANGELOG_HEADING_RE = re.compile(r'^##\s+v?([^\s(]+)')


@dataclass(frozen=True)
class ReleaseMeta:
    version: str
    tag_name: str
    tag_exists: bool
    should_release: bool
    release_body: str


def read_cargo_version(cargo_path: Path) -> str:
    in_package = False
    for raw_line in cargo_path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if line.startswith("[") and line.endswith("]"):
            in_package = line == "[package]"
            continue
        if not in_package:
            continue
        match = PACKAGE_VERSION_RE.match(line)
        if match:
            return match.group(1)
    raise ValueError(f"未在 {cargo_path} 的 [package] 区块找到 version")


def extract_latest_changelog_section(changelog_path: Path) -> tuple[str, str]:
    lines = changelog_path.read_text(encoding="utf-8").splitlines()
    start_index = next((index for index, line in enumerate(lines) if line.startswith("## ")), None)
    if start_index is None:
        raise ValueError(f"{changelog_path} 缺少版本区块")

    end_index = len(lines)
    for index in range(start_index + 1, len(lines)):
        if lines[index].startswith("## "):
            end_index = index
            break

    section_lines = lines[start_index:end_index]
    section_text = "\n".join(section_lines).strip()
    heading_match = CHANGELOG_HEADING_RE.match(section_lines[0].strip())
    if not heading_match:
        raise ValueError(f"{changelog_path} 的最新版本标题格式无效")
    return heading_match.group(1), section_text


def normalize_existing_tags(existing_tags: Iterable[str]) -> set[str]:
    return {tag.strip() for tag in existing_tags if tag and tag.strip()}


def build_release_meta(
    cargo_path: Path,
    changelog_path: Path,
    existing_tags: Iterable[str],
) -> ReleaseMeta:
    version = read_cargo_version(cargo_path)
    changelog_version, release_body = extract_latest_changelog_section(changelog_path)
    if version != changelog_version:
        raise ValueError(
            f"CHANGELOG 最新版本 {changelog_version} 与 Cargo.toml 版本 {version} 不一致"
        )

    tag_name = f"v{version}"
    tag_exists = tag_name in normalize_existing_tags(existing_tags)
    return ReleaseMeta(
        version=version,
        tag_name=tag_name,
        tag_exists=tag_exists,
        should_release=not tag_exists,
        release_body=release_body,
    )


def write_release_body(output_path: Path, release_body: str) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(release_body, encoding="utf-8")


def write_github_output(output_path: Path, meta: ReleaseMeta, body_path: Path) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        "\n".join(
            [
                f"version={meta.version}",
                f"tag_name={meta.tag_name}",
                f"tag_exists={str(meta.tag_exists).lower()}",
                f"should_release={str(meta.should_release).lower()}",
                f"release_body_path={body_path}",
            ]
        )
        + "\n",
        encoding="utf-8",
    )


def list_git_tags(repo_root: Path) -> list[str]:
    result = subprocess.run(
        ["git", "tag", "--list"],
        cwd=repo_root,
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.splitlines()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="生成 GitHub Release 元数据")
    parser.add_argument("--repo-root", default=".")
    parser.add_argument("--cargo", default="Cargo.toml")
    parser.add_argument("--changelog", default="CHANGELOG.md")
    parser.add_argument("--github-output")
    parser.add_argument("--release-body-path", default="release-body.md")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = Path(args.repo_root).resolve()
    cargo_path = (repo_root / args.cargo).resolve()
    changelog_path = (repo_root / args.changelog).resolve()
    body_path = Path(args.release_body_path).resolve()

    meta = build_release_meta(
        cargo_path=cargo_path,
        changelog_path=changelog_path,
        existing_tags=list_git_tags(repo_root),
    )

    write_release_body(body_path, meta.release_body)
    if args.github_output:
        write_github_output(Path(args.github_output).resolve(), meta, body_path)

    print(f"version={meta.version}")
    print(f"tag_name={meta.tag_name}")
    print(f"tag_exists={str(meta.tag_exists).lower()}")
    print(f"should_release={str(meta.should_release).lower()}")
    print(f"release_body_path={body_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
