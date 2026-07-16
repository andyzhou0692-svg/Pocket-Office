import importlib.util
import json
import tempfile
import unittest
from unittest import mock
from pathlib import Path


SPEC = importlib.util.spec_from_file_location(
    "gen_media", Path(__file__).with_name("gen-media.py")
)
gen_media = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(gen_media)


class MediaManifestTests(unittest.TestCase):
    def test_docs_office_media_no_longer_generates_alternate_themes(self):
        manifest = json.loads(Path(__file__).with_name("media.json").read_text())
        jobs = {job["id"]: job for job in manifest}

        self.assertEqual(jobs["themes-composite"]["kind"], "render")
        self.assertEqual(jobs["themes-composite"]["theme"], "200West")
        self.assertTrue(
            all(frame["theme"] == "200West" for frame in jobs["reference"]["frames"])
        )

    def test_hand_made_comparison_captures_are_not_owned_by_the_generator(self):
        captures = {
            "2dpig-live-200west-jess.png",
            "2dpig-live-200west-office.png",
            "2dpig-live-200west-terminal-120x31.png",
            "2dpig-live-200west-vivian.png",
            "black-transparent-glasses-200west-office.png",
            "black-transparent-glasses-amy.png",
        }

        self.assertLessEqual(captures, gen_media.NOT_GENERATED)


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


class ClipJobTests(unittest.TestCase):
    def test_run_clip_forwards_render_context_to_snapshot(self):
        calls = []
        original_snap = gen_media.snap
        gen_media.snap = lambda *args, **kwargs: calls.append(kwargs)
        self.addCleanup(setattr, gen_media, "snap", original_snap)

        job = {
            "id": "pets",
            "cols": 120,
            "rows": 52,
            "hour": 14,
            "day": 1,
            "theme": "200West",
            "weather": "clear",
            "duration": 12,
            "fps": 15,
            "extra": ["--pets", "cat"],
        }
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.object(gen_media, "ffmpeg"):
                gen_media.run_clip(job, [Path(tmp)], Path(tmp), {})

        self.assertEqual(calls[0]["day"], 1)
        self.assertEqual(calls[0]["theme"], "200West")
        self.assertEqual(calls[0]["weather"], "clear")


if __name__ == "__main__":
    unittest.main()
