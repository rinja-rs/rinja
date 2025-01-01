from unittest import main, TestCase

from jinja2 import Environment, PackageLoader


class BlockInIncludePartial(TestCase):
    def setUp(self):
        self.__env = Environment(loader=PackageLoader(__name__, "../../templates"))

    def __render(self, template, block=None) -> str:
        tmpl = self.__env.get_template(template)
        if not block:
            return tmpl.render()
        else:
            return "".join(tmpl.blocks[block](tmpl.new_context()))

    def test_block_in_include_extended(self):
        self.assertEqual(
            self.__render("block_in_include_extended.html"),
            "block_in_base: from extended!\nblock_in_partial: from partial!\n",
        )

    def test_block_in_include_base(self):
        self.assertEqual(
            self.__render("block_in_include_base.html", "block_in_base"),
            "block_in_base: from base!\n",
        )

    def test_block_in_include_partial(self):
        self.assertEqual(
            self.__render("block_in_include_partial.html", "block_in_partial"),
            "block_in_partial: from partial!\n",
        )


if __name__ == "__main__":
    main()
