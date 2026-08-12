#!/usr/bin/env python3
"""Bootstrap crates.io trusted publishing for the GPUI Box release cohort."""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any


API = "https://crates.io/api/v1"
USER_AGENT = "gpui-box-release/0.1 (https://github.com/fran0220/gpui-box)"
EXPECTED_OWNER = "fran0220"
DESIRED_CONFIG = {
    "repository_owner": "fran0220",
    "repository_name": "gpui-box",
    "workflow_filename": "release.yml",
    "environment": "crates-io",
}


class RegistryError(RuntimeError):
    pass


@dataclass(frozen=True)
class Package:
    name: str
    version: str


def packages_from_authority(path: Path) -> list[Package]:
    text = path.read_text(encoding="utf-8")
    packages: list[Package] = []
    for block in text.split("[[package]]")[1:]:
        fields: dict[str, str] = {}
        for key in ("name", "version", "publish"):
            match = re.search(rf'^{key} = (?:("[^"]*")|(true|false))$', block, re.MULTILINE)
            if match is None:
                raise RegistryError(f"package authority block has no {key}")
            fields[key] = match.group(1).strip('"') if match.group(1) else match.group(2)
        if fields["publish"] == "true":
            packages.append(Package(fields["name"], fields["version"]))
    if not packages:
        raise RegistryError("package authority has no publishable packages")
    names = [package.name for package in packages]
    if len(names) != len(set(names)):
        raise RegistryError("package authority has duplicate package names")
    return packages


class Registry:
    def __init__(self, token: str):
        if not token:
            raise RegistryError("CARGO_REGISTRY_TOKEN is required")
        self.token = token

    def request(
        self,
        method: str,
        path: str,
        *,
        payload: dict[str, Any] | None = None,
        authenticated: bool = True,
        expected: tuple[int, ...] = (200,),
    ) -> tuple[int, dict[str, Any] | None]:
        body = None if payload is None else json.dumps(payload).encode("utf-8")
        headers = {
            "Accept": "application/json",
            "User-Agent": USER_AGENT,
        }
        if authenticated:
            headers["Authorization"] = f"Bearer {self.token}"
        if body is not None:
            headers["Content-Type"] = "application/json"
        request = urllib.request.Request(
            f"{API}{path}", data=body, headers=headers, method=method
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                status = response.status
                response_body = response.read()
        except urllib.error.HTTPError as error:
            status = error.code
            response_body = error.read()
        if status not in expected:
            detail = f"HTTP {status}"
            if response_body:
                try:
                    parsed = json.loads(response_body)
                    errors = parsed.get("errors", [])
                    if errors and isinstance(errors[0], dict):
                        detail += f": {errors[0].get('detail', 'registry request failed')}"
                except (json.JSONDecodeError, AttributeError):
                    pass
            raise RegistryError(f"{method} {path} failed with {detail}")
        if not response_body:
            return status, None
        try:
            return status, json.loads(response_body)
        except json.JSONDecodeError as error:
            raise RegistryError(f"{method} {path} returned invalid JSON") from error


def quote(value: str) -> str:
    return urllib.parse.quote(value, safe="")


def desired_for(package: Package) -> dict[str, str]:
    return {"crate": package.name, **DESIRED_CONFIG}


def require_published_and_owned(registry: Registry, package: Package) -> None:
    registry.request(
        "GET",
        f"/crates/{quote(package.name)}/{quote(package.version)}",
        authenticated=False,
    )
    _, owners = registry.request(
        "GET", f"/crates/{quote(package.name)}/owners", authenticated=False
    )
    if owners is None:
        raise RegistryError(f"{package.name} owners response was empty")
    if not any(
        owner.get("login") == EXPECTED_OWNER and owner.get("kind") == "user"
        for owner in owners.get("users", [])
    ):
        raise RegistryError(
            f"{package.name} is not individually owned by {EXPECTED_OWNER}"
        )


def list_configs(registry: Registry, package: Package) -> list[dict[str, Any]]:
    _, response = registry.request(
        "GET",
        "/trusted_publishing/github_configs?crate=" + quote(package.name),
    )
    if response is None:
        raise RegistryError(f"trusted-publisher response was empty for {package.name}")
    configs = response.get("github_configs")
    total = response.get("meta", {}).get("total")
    if not isinstance(configs, list) or total != len(configs):
        raise RegistryError(f"incomplete trusted-publisher response for {package.name}")
    return configs


def exact_config(config: dict[str, Any], package: Package) -> bool:
    desired = desired_for(package)
    return all(config.get(key) == value for key, value in desired.items())


def require_one_exact_config(registry: Registry, package: Package) -> None:
    configs = list_configs(registry, package)
    if len(configs) != 1 or not exact_config(configs[0], package):
        raise RegistryError(
            f"{package.name} does not have exactly one expected trusted publisher"
        )


def configure(registry: Registry, packages: list[Package]) -> None:
    states: dict[str, str] = {}
    for package in packages:
        require_published_and_owned(registry, package)
        configs = list_configs(registry, package)
        if not configs:
            states[package.name] = "create"
        elif len(configs) == 1 and exact_config(configs[0], package):
            states[package.name] = "already configured"
        else:
            raise RegistryError(
                f"{package.name} has duplicate or conflicting trusted publishers"
            )
    for package in packages:
        if states[package.name] == "create":
            registry.request(
                "POST",
                "/trusted_publishing/github_configs",
                payload={"github_config": desired_for(package)},
            )
            states[package.name] = "created"
    for package in packages:
        require_one_exact_config(registry, package)
        print(f"{package.name}: {states[package.name]}")


def harden(registry: Registry, packages: list[Package]) -> None:
    for package in packages:
        require_one_exact_config(registry, package)
    for package in packages:
        _, response = registry.request(
            "PATCH",
            f"/crates/{quote(package.name)}",
            payload={"crate": {"trustpub_only": True}},
        )
        if response is None or response.get("crate", {}).get("trustpub_only") is not True:
            raise RegistryError(f"{package.name} did not enable trusted-publishing-only mode")
        print(f"{package.name}: trusted-publishing-only")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("configure", "harden"))
    parser.add_argument(
        "--authority",
        type=Path,
        default=Path("package-authority.toml"),
    )
    args = parser.parse_args(argv)
    if os.environ.get("GPUI_BOX_TRUSTPUB_BOOTSTRAP") != "1":
        raise RegistryError("refusing to change crates.io without bootstrap opt-in")
    packages = packages_from_authority(args.authority)
    registry = Registry(os.environ.get("CARGO_REGISTRY_TOKEN", ""))
    if args.mode == "configure":
        configure(registry, packages)
    else:
        harden(registry, packages)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except RegistryError as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)
