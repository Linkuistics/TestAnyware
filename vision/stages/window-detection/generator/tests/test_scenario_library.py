from window_gen.scenario_library import build_scenario_library
from window_gen.scenarios import WindowScenario


class TestScenarioLibrary:
    def test_library_is_not_empty(self):
        library = build_scenario_library()
        assert len(library) > 0

    def test_all_entries_are_scenarios(self):
        library = build_scenario_library()
        for scenario in library:
            assert isinstance(scenario, WindowScenario)
            assert scenario.name
            assert scenario.screen_width > 0
            assert scenario.screen_height > 0
            assert len(scenario.windows) >= 0

    def test_has_zero_window_scenario(self):
        library = build_scenario_library()
        names = [s.name for s in library]
        assert "empty-desktop" in names

    def test_has_single_window_scenario(self):
        library = build_scenario_library()
        single = [s for s in library if len(s.windows) == 1]
        assert len(single) >= 1

    def test_has_multi_window_scenario(self):
        library = build_scenario_library()
        multi = [s for s in library if len(s.windows) >= 3]
        assert len(multi) >= 1

    def test_has_overlapping_windows(self):
        library = build_scenario_library()
        has_overlap = False
        for scenario in library:
            for i, w1 in enumerate(scenario.windows):
                d1 = w1.to_detection()
                for w2 in scenario.windows[i + 1:]:
                    d2 = w2.to_detection()
                    if d1.bbox.iou(d2.bbox) > 0:
                        has_overlap = True
        assert has_overlap, "Library must include overlapping window scenarios"

    def test_windows_within_screen_bounds(self):
        library = build_scenario_library()
        for scenario in library:
            for w in scenario.windows:
                assert w.x >= 0, f"{scenario.name}: window x < 0"
                assert w.y >= 0, f"{scenario.name}: window y < 0"
                assert w.x + w.width <= scenario.screen_width
                assert w.y + w.height <= scenario.screen_height
