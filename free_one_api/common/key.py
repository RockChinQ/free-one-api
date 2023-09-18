"""Utility functions for key generation."""

import random
import string


def generate_api_key() -> str:
    """Generate API key which starts with `sk-foa`, 51 chars in total.
    """
    result = "sk-foa" + "".join(
        [random.choice(string.ascii_letters + string.digits) for _ in range(45)]
    )

    return result
