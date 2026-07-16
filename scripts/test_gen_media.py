import importlib.util
import tempfile
import unittest
from pathlib import Path


SPEC = importlib.util.spec_from_file_location(
    "gen_media", Path(__file__).with_name("gen-media.py")
)
gen_media = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(gen_media)


class GifJobTests(unittest.TestCase):
    def test_run_gif_forwards_fixture_arguments_to_snapshot(self):
        calls = []
        original_snap = gen_media.snap
        original_which = gen_media.shutil.which
        original_run = gen_media.subprocess.run
        gen_media.snap = lambda *args, **kwargs: calls.append(kwargs)
        gen_media.shutil.which = lambda _name: "/usr/local/bin/gifsicle"
        gen_media.subprocess.run = lambda *_args, **_kwargs: None
        self.addCleanup(setattr, gen_media, "snap", original_snap)
        self.addCleanup(setattr, gen_media.shutil, "which", original_which)
        self.addCleanup(setattr, gen_media.subprocess, "run", original_run)

        job = {
            "id": "demo",
            "cols": 192,
            "rows": 64,
            "hour": 15,
            "day": 1,
            "theme": "200West",
            "duration": 15,
            "fps": 10,
            "colors": 128,
            "extra": ["--meeting", "3"],
        }
        with tempfile.TemporaryDirectory() as tmp:
            gen_media.run_gif(job, [Path(tmp)], Path(tmp), {})

        self.assertEqual(calls[0]["extra"], ["--meeting", "3"])


if __name__ == "__main__":
    unittest.main()
