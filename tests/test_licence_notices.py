"""TRE-REQ-025: the wheel carries the licence notices of what it bundles.

These read the repository, not an installed wheel: they fail when someone embeds a font, or adds a
notice file, without the other half. `maturin` puts every file `license-files` lists under
`dist-info/licenses`, so the check is that the list covers `third_party/` and that the notices name
every font the engine embeds outside tests.
"""

import pathlib
import re
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent
NOTICES = ROOT / "third_party" / "THIRD_PARTY_NOTICES.md"


def embedded_fonts():
    """Font file names `include_bytes!`ed by the engine's own sources (not by `tests/`)."""
    names = set()
    for source in (ROOT / "crates").glob("*/src/**/*.rs"):
        for found in re.findall(r'include_bytes!\("[^"]*/([^"/]+\.(?:ttf|otf))"\)', source.read_text()):
            # Emoji subsets are test fixtures (their own README says so): a source-file test module may embed them.
            if "Emoji" not in found:
                names.add(found)
    return names


def test_the_wheel_metadata_lists_the_third_party_folder():
    project = tomllib.loads((ROOT / "pyproject.toml").read_text())["project"]
    assert "LICENSE" in project["license-files"]
    assert "third_party/**/*" in project["license-files"]


def test_every_embedded_font_has_a_named_family_and_its_licence_text_is_in_third_party():
    text = NOTICES.read_text()
    fonts = embedded_fonts()
    assert fonts, "the engine embeds fonts; the scan found none (did the include_bytes! path form change?)"
    for name in fonts:
        family = name.split("-")[0].replace("NerdFontMono", " Nerd Font Mono")
        family = re.sub(r"(?<=[a-z])(?=[A-Z])", " ", family)
        assert family.lower().split()[0] in text.lower(), f"{name}: no notice names {family!r}"
    for line in re.findall(r"`(fonts/[^`]+|system-libs/[^`]+)`", text):
        assert (ROOT / "third_party" / line).is_file(), f"notices cite {line}, which is missing"


def test_the_notices_name_the_crates_and_the_five_libraries_auditwheel_adds():
    text = NOTICES.read_text()
    assert re.search(r"## Rust crates \(\d{3}\)", text)
    for lib in ("bzip2", "fontconfig", "freetype", "libpng", "libuuid"):
        assert lib in text
