"""
Pytest configuration for OGR PolishMap Driver tests.

This conftest.py provides shared fixtures and configuration for all tests.
"""

import os
import sys
import pytest

# Add the parent directory to path for imports
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# Test data directory
TEST_DATA_DIR = os.path.join(os.path.dirname(__file__), "data")


@pytest.fixture
def test_data_dir():
    """Return the path to the test data directory."""
    return TEST_DATA_DIR


@pytest.fixture
def valid_minimal_dir(test_data_dir):
    """Return the path to the valid-minimal test data directory."""
    return os.path.join(test_data_dir, "valid-minimal")


@pytest.fixture
def valid_complex_dir(test_data_dir):
    """Return the path to the valid-complex test data directory."""
    return os.path.join(test_data_dir, "valid-complex")


@pytest.fixture
def edge_cases_dir(test_data_dir):
    """Return the path to the edge-cases test data directory."""
    return os.path.join(test_data_dir, "edge-cases")


@pytest.fixture
def error_recovery_dir(test_data_dir):
    """Return the path to the error-recovery test data directory."""
    return os.path.join(test_data_dir, "error-recovery")


@pytest.fixture
def performance_dir(test_data_dir):
    """Return the path to the performance test data directory."""
    return os.path.join(test_data_dir, "performance")


def pytest_configure(config):
    """Configure pytest markers."""
    config.addinivalue_line("markers", "smoke: mark test as smoke test")
    config.addinivalue_line("markers", "integration: mark test as integration test")
    config.addinivalue_line("markers", "performance: mark test as performance test")
    config.addinivalue_line("markers", "edge_case: mark test as edge case test")
    config.addinivalue_line("markers", "error_recovery: mark test as error recovery test")


def pytest_collection_modifyitems(config, items):
    """Add markers based on test file names."""
    for item in items:
        # Auto-tag based on filename
        if "performance" in item.fspath.basename:
            item.add_marker(pytest.mark.performance)
        if "edge_cases" in item.fspath.basename:
            item.add_marker(pytest.mark.edge_case)
        if "error_recovery" in item.fspath.basename:
            item.add_marker(pytest.mark.error_recovery)
        if "valid_corpus" in item.fspath.basename:
            item.add_marker(pytest.mark.integration)


# Note: pytest_collection_finish hook available for future reporting needs
# Currently, test collection is handled by pytest default behavior
