import tempfile
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).parents[1]))
from tools.source_coverage import declared_states, decomp_states, format_report, inspect


def _pinned_melee_root() -> Path:
    """Find the pinned decomp beside either local or shared project roots."""
    anchors = (Path(__file__).resolve(), Path.cwd().resolve())
    candidates = []
    for anchor in anchors:
        for ancestor in anchor.parents:
            candidates.extend(
                (
                    ancestor / "External" / "melee",
                    ancestor / "Projects" / "Code" / "External" / "melee",
                )
            )
    # The source checkout may be mirrored into /mnt/shared while this repo is
    # worked from a personal checkout under /home; keep the lookup independent
    # of the current checkout's nesting depth.
    candidates.append(Path("/mnt/shared/Projects/Code/External/melee"))
    for candidate in candidates:
        if (candidate / "src/melee").is_dir():
            return candidate
    raise AssertionError(
        "pinned melee decomp not found; searched: "
        + ", ".join(str(candidate) for candidate in candidates)
    )


class SourceCoverageTests(unittest.TestCase):
    def test_extracts_keyword_and_source_phase_literals(self):
        source = """
from fighter import action, source_phase
first = action(Action.SPECIAL_N_START, slippi_state=347)
second = source_phase(348)
ignored = action(Action.WAIT, animation=349)
"""
        self.assertEqual(declared_states(source), frozenset({347, 348}))

    def test_reads_motion_state_comments_without_treating_arbitrary_numbers_as_states(self):
        decomp = """
        // ftCa_MS_SpecialN = 347
        /* ftCa_MS_SpecialAirN = 348 */
        const int unrelated = 349;
        """
        self.assertEqual(decomp_states(decomp), frozenset({347, 348}))

    def test_reports_missing_and_extra_declarations(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            script = root / "captain.py"
            decomp = root / "captain.c"
            script.write_text("a = source_phase(347)\nb = source_phase(349)\n", encoding="utf-8")
            decomp.write_text("// state = 347\n// state = 348\n", encoding="utf-8")
            result = inspect(script, decomp)
            self.assertEqual(result.missing, frozenset({348}))
            self.assertEqual(result.extra, frozenset({349}))
            report = format_report(result)
            self.assertIn("missing: 348", report)
            self.assertIn("extra: 349", report)

    def test_captain_inventory_matches_all_pinned_motion_states(self):
        root = Path(__file__).parents[3]
        melee = _pinned_melee_root()
        result = inspect(
            root / "scripts/fighters/captain.py",
            melee / "src/melee/ft/kinds/ftCaptain/ftcaptain.c",
        )
        self.assertEqual(result.upstream, frozenset(range(341, 364)))
        self.assertEqual(result.missing, frozenset(range(341, 347)))
        self.assertEqual(result.extra, frozenset())


if __name__ == "__main__":
    unittest.main()
