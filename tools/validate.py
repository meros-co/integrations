#!/usr/bin/env python3
"""Validate every device spec against the JSON Schema, plus the cross-field
rules a schema cannot express.

    python tools/validate.py

Exits non-zero on any failure, so it can gate CI.
"""
from __future__ import annotations

import glob
import json
import sys
from pathlib import Path

try:
    import yaml
    import jsonschema
except ImportError as exc:  # pragma: no cover
    sys.exit(f"missing dependency: {exc}. pip install pyyaml jsonschema")

ROOT = Path(__file__).resolve().parent.parent


def cross_field_checks(doc: dict, path: str) -> list[str]:
    """Rules the schema cannot express on its own."""
    errors: list[str] = []
    commands = set((doc.get("commands") or {}).keys())
    native = doc.get("implementation") == "native"

    if native:
        if not doc.get("reason"):
            errors.append("implementation: native requires a 'reason'")
        if commands:
            errors.append("implementation: native must not declare commands")
    else:
        if not doc.get("transport"):
            errors.append("a spec-implemented device requires a 'transport'")
        if not commands:
            errors.append("a spec-implemented device requires at least one command")

    model_ids = set()
    for model in doc.get("models", []):
        model_ids.add(model["id"])

        # Every supported command must exist.
        unknown = set(model.get("supports", [])) - commands
        if unknown and not native:
            errors.append(
                f"model '{model['id']}' claims unknown command(s): {sorted(unknown)}"
            )

        # Verification is a promise about evidence — check the vectors exist.
        if model.get("verification") in ("bench", "field"):
            vector_dir = ROOT / "vectors" / doc["id"]
            if not vector_dir.is_dir() or not any(vector_dir.glob("*.yaml")):
                errors.append(
                    f"model '{model['id']}' claims verification "
                    f"'{model['verification']}' but no vectors exist in "
                    f"vectors/{doc['id']}/ — see CONTRIBUTING.md"
                )

    # Quirks must reference real models (or the literal 'all').
    for i, quirk in enumerate(doc.get("quirks", [])):
        bad = {m for m in quirk["models"] if m != "all" and m not in model_ids}
        if bad:
            errors.append(f"quirk[{i}] references unknown model(s): {sorted(bad)}")

    return errors


def main() -> int:
    schema = json.loads((ROOT / "schema" / "device-spec-1.json").read_text("utf-8"))
    files = sorted(glob.glob(str(ROOT / "devices" / "*.yaml")))
    if not files:
        print("no specs found in devices/")
        return 1

    failed = False
    seen_ids: dict[str, str] = {}

    for path in files:
        rel = Path(path).relative_to(ROOT).as_posix()
        doc = yaml.safe_load(Path(path).read_text("utf-8"))

        try:
            jsonschema.validate(doc, schema)
        except jsonschema.ValidationError as exc:
            failed = True
            print(f"INVALID  {rel}")
            print(f"    path: {list(exc.absolute_path)}")
            print(f"    {exc.message}")
            continue

        errors = cross_field_checks(doc, rel)

        # Spec ids are the consumer-facing contract; they must be unique.
        spec_id = doc["id"]
        if spec_id in seen_ids:
            errors.append(f"duplicate spec id '{spec_id}' (also in {seen_ids[spec_id]})")
        seen_ids[spec_id] = rel

        if errors:
            failed = True
            print(f"INVALID  {rel}")
            for err in errors:
                print(f"    {err}")
        else:
            models = len(doc.get("models", []))
            cmds = len(doc.get("commands") or {})
            print(f"VALID    {rel}  ({models} models, {cmds} commands)")

    print("---")
    print("FAILURES PRESENT" if failed else f"all {len(files)} specs valid")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
