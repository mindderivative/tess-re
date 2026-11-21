"""0.6.12: a `path` node's `shadows` follow the path as it is filled, holes included.

Before, every node's shadow was its bounding box blurred, so a ring's hole showed a solid shadow.
Pixels come from `Window.snapshot()`.
"""

from tess_re import Window

BACK = (200, 200, 200, 255)
FILL = (40, 40, 60, 255)
# Outer clockwise, inner counter-clockwise: a ring 20 wide in a 200x120 box.
RING = "M0,0 L200,0 L200,120 L0,120 Z M20,20 L20,100 L180,100 L180,20 Z"


def pixel(shot, x, y):
    rgba, width, _ = shot
    i = (y * width + x) * 4
    return tuple(rgba[i : i + 4])


def ring(**props):
    window = Window(width=200, height=120)
    window.root.set(fill=BACK)
    path = window.create("path", data=RING, width=200, height=120, x=0, y=0,
                         position="absolute", fill=FILL, **props)
    window.root.add_child(path)
    return window, path


def test_the_ring_without_a_shadow_fills_its_band_and_leaves_its_hole():
    shot = ring()[0].snapshot()
    assert [pixel(shot, x, 60) for x in (0, 10, 19)] == [FILL] * 3
    assert [pixel(shot, x, 60) for x in (20, 100, 179)] == [BACK] * 3
    assert [pixel(shot, x, 60) for x in (180, 190, 199)] == [FILL] * 3
    assert pixel(shot, 100, 10) == FILL


def test_a_blurred_black_shadow_does_not_fill_the_hole():
    window, _ = ring(shadows=[((0, 0, 0, 255), 0, 0, 16, 0)])
    shot = window.snapshot()
    # The reported repro: the centre was (0, 0, 0, 255), a solid box.
    assert pixel(shot, 100, 60) == BACK
    # The hole is darkened only beside the band, more the closer to it.
    near, mid = pixel(shot, 24, 60), pixel(shot, 40, 60)
    assert near[0] < mid[0] < BACK[0], (near, mid)
    assert pixel(shot, 60, 60) == BACK
    # The band paints over its own shadow.
    assert pixel(shot, 10, 60) == FILL


def test_a_hard_shadow_shifted_into_the_hole_is_the_band_not_the_box():
    red = (255, 0, 0, 255)
    window, path = ring(shadows=[(red, 60, 0, 0, 0)])
    shot = window.snapshot()
    assert pixel(shot, 70, 60) == red  # the left band, moved 60 right
    assert pixel(shot, 100, 60) == BACK  # its own hole
    assert pixel(shot, 130, 60) == BACK
    path.set(shadows=[])
    assert pixel(window.snapshot(), 70, 60) == BACK


def test_a_positive_spread_grows_the_band_into_the_hole():
    red = (255, 0, 0, 255)
    window, _ = ring(shadows=[(red, 0, 0, 0, 6)])
    shot = window.snapshot()
    assert pixel(shot, 25, 60) == red  # 6 into the hole
    assert pixel(shot, 30, 60) == BACK
