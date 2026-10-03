import json
import unittest

from prepare_release_notes import render, validate


class ReleaseNotesTest(unittest.TestCase):
    def note(self):
        return {"version": "0.2.1-a", "ru": {"title": "Исправление", "changes": ["Текст виден."]},
                "en": {"title": "Fix", "changes": ["Text is visible."]}}

    def test_machine_notes_match_the_readable_notes_and_actual_tag(self):
        note = self.note()
        body = render(note, "v0.2.1a")
        self.assertIn("/v0.2.1a/Fastcloud_0.2.1-a_x64-setup.exe", body.splitlines()[0])
        machine = body.split("<!-- fastcloud-notes:v1 ")[1].split(" -->")[0]
        self.assertEqual(json.loads(machine), note)
        self.assertIn("- Текст виден.", body)
        self.assertIn("- Text is visible.", body)

    def test_missing_translation_and_duplicates_fail(self):
        note = self.note()
        with self.assertRaises(ValueError):
            validate({"schema": 1, "releases": [note, note]})
        del note["en"]
        with self.assertRaises(ValueError):
            validate({"schema": 1, "releases": [note]})

    def test_comment_injection_fails(self):
        note = self.note()
        note["en"]["changes"] = ["--> <script>alert(1)</script>"]
        with self.assertRaises(ValueError):
            validate({"schema": 1, "releases": [note]})


if __name__ == "__main__":
    unittest.main()
