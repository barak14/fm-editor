"""Host-independent tests for the release startup gate; no GUI or game required."""

import importlib.util
from pathlib import Path
import subprocess
import unittest
from unittest.mock import Mock, patch


spec = importlib.util.spec_from_file_location(
    "smoke_release", Path(__file__).parents[1] / "smoke-release.py"
)
smoke_release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke_release)


class SmokeReleaseTests(unittest.TestCase):
    def process(self, status=None):
        process = Mock()
        process.poll.return_value = status
        return process

    def test_survival_terminates_test_process(self):
        process = self.process()
        with patch.object(smoke_release, "mcp_port_open", return_value=False), patch.object(
            smoke_release.subprocess, "Popen", return_value=process
        ) as launch, patch.object(smoke_release.time, "sleep"):
            smoke_release.smoke(["editor"])
        launch.assert_called_once()
        process.terminate.assert_called_once()
        process.wait.assert_called_once_with(timeout=5)

    def test_early_exit_including_zero_is_failure(self):
        for status in (0, 1):
            with self.subTest(status=status):
                process = self.process(status)
                with patch.object(smoke_release, "mcp_port_open", return_value=False), patch.object(
                    smoke_release.subprocess, "Popen", return_value=process
                ), patch.object(smoke_release.time, "sleep"):
                    with self.assertRaisesRegex(RuntimeError, "exited during startup"):
                        smoke_release.smoke(["editor"])
                process.terminate.assert_not_called()

    def test_existing_mcp_port_prevents_launch(self):
        with patch.object(smoke_release, "mcp_port_open", return_value=True), patch.object(
            smoke_release.subprocess, "Popen"
        ) as launch:
            with self.assertRaisesRegex(RuntimeError, "already in use"):
                smoke_release.smoke(["editor"])
        launch.assert_not_called()

    def test_new_mcp_listener_is_failure_and_still_terminates(self):
        process = self.process()
        with patch.object(smoke_release, "mcp_port_open", side_effect=[False, True]), patch.object(
            smoke_release.subprocess, "Popen", return_value=process
        ), patch.object(smoke_release.time, "sleep"):
            with self.assertRaisesRegex(RuntimeError, "opened the debug MCP"):
                smoke_release.smoke(["editor"])
        process.terminate.assert_called_once()

    def test_termination_timeout_kills_process(self):
        process = self.process()
        process.wait.side_effect = [subprocess.TimeoutExpired("editor", 5), None]
        with patch.object(smoke_release, "mcp_port_open", return_value=False), patch.object(
            smoke_release.subprocess, "Popen", return_value=process
        ), patch.object(smoke_release.time, "sleep"):
            smoke_release.smoke(["editor"])
        process.kill.assert_called_once()
        self.assertEqual(process.wait.call_count, 2)

    def test_nonpositive_observation_time_rejected(self):
        with self.assertRaises(ValueError):
            smoke_release.smoke(["editor"], seconds=0)


if __name__ == "__main__":
    unittest.main()
