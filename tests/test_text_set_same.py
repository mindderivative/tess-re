"""T-037: `set(text=...)` with the text the input already holds leaves the caret and selection alone."""

from helpers import add

from tess_re import Window


def field(text="hello world"):
    window = Window(width=300, height=100)
    node = add(window, "text_input", width=200, height=30, text=text)
    window.simulate("focus", node=node)
    return window, node


def test_typing_in_the_middle_survives_a_two_way_binding():
    window, node = field()
    changes = []
    node.on("change", lambda e: changes.append(node.get("text")))
    window.simulate("key_down", key="home")
    for ch in "AB":
        window.simulate("input", text=ch)
        node.set(text=node.get("text"))  # the framework echoes the text back
    assert node.get("text") == "ABhello world"
    assert node.get("selection") is None or node.get("selection")[0] == node.get("selection")[1]
    assert changes == ["Ahello world", "ABhello world"], "the echo fires no change"


def test_the_same_text_keeps_a_selection():
    _, node = field()
    node.set(selection=(2, 5))
    node.set(text="hello world")
    assert node.get("selection") == (2, 5)


def test_a_different_text_puts_the_caret_at_its_end_and_clears_the_selection():
    window, node = field()
    node.set(selection=(2, 5))
    node.set(text="other")
    window.simulate("input", text="!")
    assert node.get("text") == "other!"
