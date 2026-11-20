"""0.6.10 (TRE-REQ-006): `repeat` and `spread` on `Gradient.linear`, `radial` and `sweep`.

Pixel checks on a headless snapshot: the ramp runs black -> red, so the red channel says where in
the ramp a pixel is (0 at the start of a pass, 255 at its end).
"""

import pytest

from tess_re import Gradient, Window

BLACK = (0, 0, 0, 255)
RED = (255, 0, 0, 255)
SIZE = 100


def shot(fill):
    window = Window(width=SIZE, height=SIZE)
    window.root.set(padding=0)  # the root's default padding would shrink the box
    window.root.add_child(window.create("box", width=SIZE, height=SIZE, fill=fill))
    rgba, width, _ = window.snapshot()

    def red(x, y):
        return rgba[(y * width + x) * 4]

    return red


def test_one_pass_is_unchanged_by_the_new_arguments():
    plain = shot(Gradient.linear([BLACK, RED], angle=90))
    explicit = shot(Gradient.linear([BLACK, RED], angle=90, repeat=1.0, spread="pad"))
    assert [plain(x, 50) for x in range(0, 100, 7)] == [explicit(x, 50) for x in range(0, 100, 7)]
    assert plain(50, 50) == pytest.approx(128, abs=8) and plain(97, 50) > 230


def test_a_linear_ramp_repeats_along_its_line():
    red = shot(Gradient.linear([BLACK, RED], angle=90, repeat=2))  # two 50px passes, left to right
    assert red(3, 50) < 30 and red(46, 50) > 220
    assert red(54, 50) < 40, "the second pass starts again from black"
    assert red(96, 50) > 220
    assert red(25, 50) == pytest.approx(128, abs=16) and red(75, 50) == pytest.approx(128, abs=16)


def test_a_fractional_repeat_leaves_the_last_pass_partial():
    red = shot(Gradient.linear([BLACK, RED], angle=90, repeat=1.5))  # 66.7px passes
    assert red(60, 50) > 200, "the first pass is nearly at red here"
    assert red(70, 50) < 40, "the second begins at 66.7px"
    assert red(97, 50) == pytest.approx(128, abs=24), "and is half done at the far edge"


def test_reflect_runs_the_ramp_back_on_the_second_pass():
    red = shot(Gradient.linear([BLACK, RED], angle=90, repeat=2, spread="reflect"))
    assert red(3, 50) < 30 and red(46, 50) > 220
    assert red(54, 50) > 220, "the second pass runs back from red"
    assert red(96, 50) < 30


def test_a_radial_ramp_repeats_in_rings():
    red = shot(Gradient.radial([BLACK, RED], repeat=3))  # rings 23.6px wide
    assert red(52, 50) < 30 and red(50 + 21, 50) > 200, "red at the end of the first ring"
    assert red(50 + 27, 50) < 80, "black again at the start of the second"


def test_a_sweep_ramp_repeats_around_the_turn():
    red = shot(Gradient.sweep([BLACK, RED], repeat=2))  # two 180 degree passes, clockwise from up
    assert red(52, 10) < 30, "just clockwise of up is the start of the first pass"
    assert red(90, 50) == pytest.approx(128, abs=16), "three o'clock is half of it"
    assert red(53, 90) > 220, "just before six o'clock is the end of the first pass"
    assert red(46, 90) < 40, "just after it, the second pass starts again"


def test_the_default_spread_follows_repeat_and_is_readable():
    assert (Gradient.linear([BLACK, RED]).repeat, Gradient.linear([BLACK, RED]).spread) == (1.0, "pad")
    assert Gradient.sweep([BLACK, RED], repeat=4).spread == "repeat"
    assert Gradient.radial([BLACK, RED], repeat=2, spread="reflect").spread == "reflect"
    assert "repeat=2" in repr(Gradient.linear([BLACK, RED], repeat=2))
    assert Gradient.linear([BLACK, RED], repeat=2) != Gradient.linear([BLACK, RED])


@pytest.mark.parametrize("repeat", [0, -1, float("nan"), float("inf")])
def test_a_repeat_must_be_a_positive_number(repeat):
    for make in (Gradient.linear, Gradient.radial, Gradient.sweep):
        with pytest.raises(ValueError, match="repeat"):
            make([BLACK, RED], repeat=repeat)


def test_an_unknown_spread_names_the_valid_ones():
    with pytest.raises(ValueError, match="pad, repeat, reflect"):
        Gradient.linear([BLACK, RED], spread="mirror")


def test_the_repeat_count_animates():
    window = Window(width=SIZE, height=SIZE)
    window.root.set(padding=0)
    box = window.create("box", width=SIZE, height=SIZE, fill=Gradient.linear([BLACK, RED], angle=90))
    window.root.add_child(box)
    box.animate("fill", Gradient.linear([BLACK, RED], angle=90, repeat=3), 1000)
    window.advance(500)
    assert box.get("fill").repeat == pytest.approx(2.0, abs=0.05)
    window.advance(600)
    assert box.get("fill").repeat == 3.0
