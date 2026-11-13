#!/usr/bin/env python3
"""Reference-based Gambatte evaluator.

This script never treats a generated image as a reference. It records one
result per ROM/model and only emits PASS/FAIL for an existing PNG reference or
the documented audio contract. Hexadecimal tile-output tests remain PENDING
until their glyph comparator is implemented.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SUITE = ROOT / "gb-test-roms-master" / "gambatte"
OUT = ROOT / ".debug" / "visual-comparison" / "gambatte"
RUNNER = Path(os.environ.get("FOX_CARGO_TARGET", "/home/sgmun/cargo")) / "release" / "gambatte-runner"

# Gambatte's original testrunner renders the sixteen hexadecimal glyphs as
# 8x8 tiles and compares only the high five bits of each RGB component.
_HEX_GLYPHS = (
    ("00000000", "01111111", "01000001", "01000001", "01000001", "01000001", "01000001", "01111111"),
    ("00000000", "00001000", "00001000", "00001000", "00001000", "00001000", "00001000", "00001000"),
    ("00000000", "01111111", "00000001", "00000001", "01111111", "01000000", "01000000", "01111111"),
    ("00000000", "01111111", "00000001", "00000001", "00111111", "00000001", "00000001", "01111111"),
    ("00000000", "01000001", "01000001", "01000001", "01111111", "00000001", "00000001", "00000001"),
    ("00000000", "01111111", "01000000", "01000000", "01111110", "00000001", "00000001", "01111110"),
    ("00000000", "01111111", "01000000", "01000000", "01111110", "01000001", "01000001", "01111110"),
    ("00000000", "01111111", "00000001", "00000010", "00000100", "00001000", "00001000", "00001000"),
    ("00000000", "00111110", "01000001", "01000001", "00111110", "01000001", "01000001", "00111110"),
    ("00000000", "01111111", "01000001", "01000001", "01111111", "00000001", "00000001", "01111111"),
    ("00000000", "00111110", "01000001", "01000001", "01111111", "01000001", "01000001", "01000001"),
    ("00000000", "01111110", "01000001", "01000001", "01111110", "01000001", "01000001", "01111110"),
    ("00000000", "00111110", "01000001", "01000000", "01000000", "01000000", "01000001", "00111110"),
    ("00000000", "01111110", "01000001", "01000001", "01000001", "01000001", "01000001", "01111110"),
    ("00000000", "01111111", "01000000", "01000000", "01111110", "01000000", "01000000", "01111110"),
    ("00000000", "01111111", "01000000", "01000000", "01111110", "01000000", "01000000", "01000000"),
)


def read_ppm(path: Path) -> list[tuple[int, int, int]]:
    data = path.read_bytes()
    header_end = data.find(b"\n255\n")
    if header_end < 0:
        raise ValueError(f"invalid PPM header: {path}")
    payload = data[header_end + len(b"\n255\n") :]
    if len(payload) != 160 * 144 * 3:
        raise ValueError(f"unexpected PPM size: {path}")
    return [tuple(payload[i : i + 3]) for i in range(0, len(payload), 3)]


def glyph_result(ppm: Path, output: str) -> tuple[bool, int]:
    pixels = read_ppm(ppm)
    differing = 0
    for tile_index, char in enumerate(output):
        value = int(char, 16)
        for y, row in enumerate(_HEX_GLYPHS[value]):
            for x, bit in enumerate(row):
                expected = 0x00 if bit == "1" else 0xF8
                actual = pixels[y * 160 + tile_index * 8 + x]
                if any((channel & 0xF8) != expected for channel in actual):
                    differing += 1
    return differing == 0, differing


def models_for(name: str) -> list[tuple[str, str | None]]:
    """Return (model, expected audio token or None) from Gambatte suffixes."""
    stem = Path(name).stem
    pairs: list[tuple[str, str | None]] = []
    combined = re.search(r"_dmg08_cgb04c_out(audio[01]|[0-9A-Fa-f]+)", stem)
    if combined:
        value = combined.group(1)
        return [("dmg", value if value.startswith("audio") else None),
                ("cgb", value if value.startswith("audio") else None)]
    split = re.search(
        r"_dmg08_out(audio[01]|[0-9A-Fa-f]+)_cgb04c_out(audio[01]|[0-9A-Fa-f]+)",
        stem,
    )
    if split:
        return [("dmg", split.group(1) if split.group(1).startswith("audio") else None),
                ("cgb", split.group(2) if split.group(2).startswith("audio") else None)]
    for model, token in (("dmg", "dmg08"), ("cgb", "cgb04c")):
        match = re.search(rf"_{token}_out(audio[01]|[0-9A-Fa-f]+)", stem)
        if match:
            value = match.group(1)
            pairs.append((model, value if value.startswith("audio") else None))
    if not pairs and "_outaudio" in stem:
        pairs.append(("cgb", re.search(r"_out(audio[01])", stem).group(1)))
    if not pairs and "_out" in stem:
        pairs.append(("cgb", None))
    return pairs


def png_reference(rom: Path, model: str) -> Path | None:
    for suffix in (
        "_dmg08_cgb04c.png" if model == "dmg" else "_dmg08_cgb04c.png",
        "_dmg08.png" if model == "dmg" else "_cgb04c.png",
    ):
        candidate = rom.with_suffix("").with_name(rom.stem + suffix)
        if candidate.exists():
            return candidate
    return None


def run_one(rom: Path, model: str, expected_audio: str | None) -> dict:
    OUT.mkdir(parents=True, exist_ok=True)
    key = rom.relative_to(ROOT).as_posix().replace("/", "__")
    ppm = OUT / f"{key}.{model}.ppm"
    command = [str(RUNNER), str(rom), model, str(ppm), "15"]
    process = subprocess.run(command, capture_output=True, text=True, timeout=30)
    line = process.stdout.strip().splitlines()[-1] if process.stdout.strip() else ""
    record = {
        "path": rom.relative_to(ROOT).as_posix(),
        "sha256": hashlib.sha256(rom.read_bytes()).hexdigest(),
        "suite": "gambatte",
        "model": model,
        "source": "gambatte-runner",
        "command": command,
        "stdout": line,
    }
    if process.returncode != 0:
        record["status"] = "ERROR"
        return record
    if expected_audio is not None:
        expected_nonconstant = expected_audio == "audio1"
        actual_nonconstant = "audio_nonconstant=true" in line
        record["status"] = "PASS" if actual_nonconstant == expected_nonconstant else "FAIL"
        record["criterion"] = {
            "kind": "audio",
            "expected_nonconstant": expected_nonconstant,
            "actual_nonconstant": actual_nonconstant,
        }
        return record
    reference = png_reference(rom, model)
    marker = re.search(r"_out([0-9A-Fa-f]+)(?:$|\.)", rom.stem)
    if marker is not None:
        output = marker.group(1)
        matches, differing = glyph_result(ppm, output)
        record["status"] = "PASS" if matches else "FAIL"
        record["criterion"] = {
            "kind": "hexadecimal-glyphs",
            "expected": output,
            "different_pixels": differing,
        }
        return record
    if reference is None:
        record["status"] = "PENDING"
        record["criterion"] = {"kind": "unclassified", "reference": None}
        return record
    metric = subprocess.run(
        ["compare", "-metric", "AE", str(reference), str(ppm), "null:"],
        capture_output=True,
        text=True,
    )
    metric_text = (metric.stderr or metric.stdout).strip()
    try:
        differing_pixels = int(metric_text.split()[0])
    except (ValueError, IndexError):
        differing_pixels = None
    record["status"] = "PASS" if metric.returncode == 0 else "FAIL"
    record["criterion"] = {
        "kind": "png-exact",
        "reference": reference.relative_to(ROOT).as_posix(),
        "different_pixels": differing_pixels,
    }
    return record


def main() -> int:
    target = (Path(sys.argv[1]) if len(sys.argv) > 1 else SUITE).resolve()
    OUT.mkdir(parents=True, exist_ok=True)
    roms = sorted(target.rglob("*.gb*")) if target.is_dir() else [target]
    roms = [p for p in roms if p.suffix.lower() in {".gb", ".gbc"} and "oam_bug" not in p.parts]
    with (OUT / "results.jsonl").open("w") as output:
        for rom in roms:
            models = models_for(rom.name)
            if not models:
                models = []
                if rom.with_suffix("").with_name(rom.stem + "_dmg08.png").exists():
                    models.append(("dmg", None))
                if rom.with_suffix("").with_name(rom.stem + "_cgb04c.png").exists():
                    models.append(("cgb", None))
            for model, audio in models:
                record = run_one(rom, model, audio)
                output.write(json.dumps(record, sort_keys=True) + "\n")
                print(record["status"], record["path"], model)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
