#!/usr/bin/env python3
"""
Version bumping script for nmrs.

This script updates version numbers:
- nmrs/Cargo.toml
- CHANGELOG.md
- Cargo.lock

Usage:
    python3 scripts/bump_version.py <version> <release_type>
"""

import re
import subprocess
import sys
from datetime import datetime
from pathlib import Path


BREAKING_MARKER = re.compile(r'\*\*breaking:?\*\*|^#+\s*breaking\b', re.IGNORECASE | re.MULTILINE)


def read_current_version(cargo_toml_path: Path) -> str | None:
    """Read the current version out of Cargo.toml."""
    match = re.search(r'^version\s*=\s*"([^"]+)"', cargo_toml_path.read_text(), re.MULTILINE)
    return match.group(1) if match else None


def unreleased_section(changelog_path: Path) -> str:
    """Return the body of the [Unreleased] section, or '' if absent."""
    match = re.search(r'## \[Unreleased\](.*?)(?=## \[|\Z)', changelog_path.read_text(), re.DOTALL)
    return match.group(1) if match else ''


def check_breaking_changes(changelog_path: Path, current: str, new: str, allow: bool) -> bool:
    """Refuse a non-major bump when [Unreleased] documents a breaking change.

    cargo-semver-checks does not catch every break (function return types, for
    one), and the version is bumped in a separate commit after CI runs, so it
    never evaluates the version actually being published. The changelog is the
    reliable signal: breaking changes are already labelled there by hand. This
    is what 3.4.0 needed and did not have.
    """
    if not BREAKING_MARKER.search(unreleased_section(changelog_path)):
        return True

    cur_major, cur_minor, _ = (int(p) for p in current.split('.'))
    new_major, new_minor, _ = (int(p) for p in new.split('.'))

    if new_major > cur_major:
        return True

    bump = 'minor' if new_minor > cur_minor else 'patch'
    print(f"✗ [Unreleased] documents a breaking change, but {current} -> {new} is a {bump} bump.")
    print()
    print("  Cargo auto-upgrades minor and patch releases, so this would break")
    print("  existing builds without opt-in. Pick one:")
    print()
    print(f"    - Release it as {cur_major + 1}.0.0")
    print("    - Make the change additive (new API + #[deprecated] on the old one)")
    print("    - Park it in the next major's milestone")
    print()
    if allow:
        print("--allow-breaking set; continuing anyway.")
        return True
    print("  Override with --allow-breaking if this is genuinely intended.")
    return False


def update_cargo_toml(file_path: Path, version: str) -> bool:
    """Update version in a Cargo.toml file.

    Already being at `version` is success, not failure: a run that dies partway
    through (a missing CHANGELOG, say) leaves Cargo.toml bumped, and treating
    the no-op re-run as an error made the resumed run fail forever - and skip
    the Cargo.lock refresh with it.
    """
    try:
        content = file_path.read_text()
        pattern = r'^version\s*=\s*"[^"]*"'
        replacement = f'version = "{version}"'

        new_content = re.sub(pattern, replacement, content, count=1, flags=re.MULTILINE)

        if new_content != content:
            file_path.write_text(new_content)
            print(f"✓ Updated {file_path}")
            return True

        if read_current_version(file_path) == version:
            print(f"Already at {version}: {file_path}")
            return True

        print(f"✗ No `version = \"...\"` line found in {file_path}")
        return False
    except Exception as e:
        print(f"✗ Error updating {file_path}: {e}")
        return False


def update_cargo_lock(project_root: Path, version: str) -> bool:
    """Refresh Cargo.lock so it records the new workspace version.

    Bumping only Cargo.toml leaves the lock pinned to the previous version, and
    every job that passes `--locked` then fails with "cannot update the lock
    file ... because --locked was passed". That is what turned the 3.5.0 release
    commit red across four CI jobs.

    `--workspace` re-resolves only workspace members, so third-party
    dependencies keep their currently locked versions.
    """
    lock_path = project_root / 'Cargo.lock'
    if not lock_path.exists():
        print(f"No Cargo.lock at {lock_path}; skipping refresh")
        return True

    try:
        result = subprocess.run(
            ['cargo', 'update', '--workspace', '--offline'],
            cwd=project_root,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError:
        print("✗ `cargo` not found on PATH; cannot refresh Cargo.lock")
        print("  Run `cargo update --workspace` manually before committing.")
        return False

    if result.returncode != 0:
        print("✗ Failed to refresh Cargo.lock")
        print(f"  {result.stderr.strip().splitlines()[-1] if result.stderr.strip() else ''}")
        print("  Run `cargo update --workspace` manually before committing.")
        return False

    if not re.search(rf'^name = "nmrs"\nversion = "{re.escape(version)}"$',
                     lock_path.read_text(), re.MULTILINE):
        print(f"✗ Cargo.lock still does not record nmrs {version}")
        return False

    print(f"✓ Updated {lock_path}")
    return True


def update_changelog(file_path: Path, version: str, release_type: str) -> bool:
    """Update CHANGELOG.md: move Unreleased to new version section."""
    try:
        content = file_path.read_text()
        today = datetime.now().strftime("%Y-%m-%d")

        unreleased_pattern = r'## \[Unreleased\](.*?)(?=## \[|\Z)'
        match = re.search(unreleased_pattern, content, re.DOTALL)

        if not match:
            print(f"No [Unreleased] section found in {file_path}")
            return False

        unreleased_content = match.group(1).strip()

        if not unreleased_content:
            print(f"[Unreleased] section is empty in {file_path}")
            unreleased_content = "\n\n(No changes documented)"

        if release_type == "stable":
            version_header = f"## [{version}] - {today}"
            version_tag = version
        else:
            version_header = f"## [{version}-{release_type}] - {today}"
            version_tag = f"{version}-{release_type}"

        new_version_section = f"{version_header}\n{unreleased_content}\n\n"
        new_unreleased_section = "## [Unreleased]\n\n"

        new_content = re.sub(
            unreleased_pattern,
            new_unreleased_section + new_version_section,
            content,
            flags=re.DOTALL
        )

        git_tag = f"nmrs-v{version_tag}"

        unreleased_link_pattern = r'\[Unreleased\]:\s*https://github\.com/[^/]+/[^/]+/compare/([^\s]+)\.\.\.HEAD'
        prev_match = re.search(unreleased_link_pattern, new_content, flags=re.IGNORECASE)
        prev_tag = prev_match.group(1).strip() if prev_match else "v0.1.0-beta"

        unreleased_link_replacement = f'[Unreleased]: https://github.com/freedesktop-rs/nmrs/compare/{git_tag}...HEAD'
        new_content = re.sub(unreleased_link_pattern, unreleased_link_replacement, new_content, flags=re.IGNORECASE)

        link_label = version if release_type == "stable" else version_tag
        new_version_link = f'[{link_label}]: https://github.com/freedesktop-rs/nmrs/compare/{prev_tag}...{git_tag}\n'

        new_content = re.sub(
            r'(\[Unreleased\]:[^\n]*\n)',
            r'\1' + new_version_link,
            new_content,
            count=1
        )

        file_path.write_text(new_content)
        print(f"✓ Updated {file_path}")
        return True
    except Exception as e:
        print(f"✗ Error updating {file_path}: {e}")
        import traceback
        traceback.print_exc()
        return False


def main():
    """Main entry point."""
    if len([a for a in sys.argv[1:] if not a.startswith('--')]) < 2:
        print("Usage: bump_version.py <version> <release_type> [--allow-breaking]")
        print()
        print("Arguments:")
        print("  version       Version number (e.g., 1.2.0)")
        print("  release_type  'beta' or 'stable'")
        print()
        print("Examples:")
        print("  python3 scripts/bump_version.py 3.1.0 stable")
        print("  python3 scripts/bump_version.py 3.1.0 beta")
        print()
        print("Flags:")
        print("  --allow-breaking  Permit a non-major bump despite a breaking")
        print("                    change in [Unreleased]. Use deliberately.")
        print()
        print("This script should be run on the dev branch before creating a PR to master.")
        sys.exit(1)

    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    flags = {a for a in sys.argv[1:] if a.startswith('--')}
    allow_breaking = '--allow-breaking' in flags

    version = args[0]
    release_type = args[1]

    if not re.match(r'^\d+\.\d+\.\d+$', version):
        print(f"✗ Invalid version format: {version}")
        print("Expected format: X.Y.Z (e.g., 1.2.0)")
        sys.exit(1)

    if release_type not in ['beta', 'stable']:
        print(f"✗ Invalid release type: {release_type}")
        print("Expected: 'beta' or 'stable'")
        sys.exit(1)

    script_dir = Path(__file__).parent
    project_root = script_dir.parent

    print(f"Preparing nmrs release: {version}-{release_type}")
    print("=" * 50)

    success = True

    cargo_toml_path = project_root / 'nmrs' / 'Cargo.toml'
    changelog_path = project_root / 'CHANGELOG.md'

    if release_type == 'stable' and cargo_toml_path.exists() and changelog_path.exists():
        current = read_current_version(cargo_toml_path)
        if current and not check_breaking_changes(changelog_path, current, version, allow_breaking):
            sys.exit(1)

    if not cargo_toml_path.exists():
        print(f"✗ File not found: {cargo_toml_path}")
        success = False
    else:
        if not update_cargo_toml(cargo_toml_path, version):
            success = False

    if not changelog_path.exists():
        print(f"✗ File not found: {changelog_path}")
        print("  Create CHANGELOG.md with an [Unreleased] section first")
        success = False
    else:
        if not update_changelog(changelog_path, version, release_type):
            success = False

    if success and not update_cargo_lock(project_root, version):
        success = False

    print("=" * 50)

    if success:
        if release_type == "stable":
            version_tag = version
        else:
            version_tag = f"{version}-{release_type}"

        git_tag = f"nmrs-v{version_tag}"

        print(f"✓ Successfully prepared nmrs release {version}-{release_type}")
        print()
        print("Next steps:")
        print(f"  1. Review the changes: git diff")
        print(f"  2. Commit: git commit -am 'chore(nmrs): prepare {version_tag} release'")
        print(f"  3. Push and open PR to master")
        print(f"  4. After merge, create tag: git tag {git_tag} && git push origin {git_tag}")
        print(f"  5. CI will automatically publish to crates.io and create GitHub release")
    else:
        print("✗ Some errors occurred during version bumping")
        sys.exit(1)


if __name__ == '__main__':
    main()
