import json

from . import llm


# Utility for adapter management

adapters: dict[str, llm.LLMLibAdapter] = {}
"""Registered adapters.

{
    "name": LLMLibAdapter
}
"""


def llm_adapter(cls: llm.LLMLibAdapter) -> llm.LLMLibAdapter:
    """Get adapter by name."""

    adapters[cls.name()] = cls

    return cls


def list_adapters() -> list[str]:
    """List all adapters."""
    return [
        {
            "name": name,
            "config_comment": cls.config_comment()
        } for name, cls in adapters.items()
    ]


def load_adapter(data: dict) -> llm.LLMLibAdapter:
    """Load adapter from dict."""
    obj = data

    return adapters[obj["type"]](obj["config"])


def dump_adapter(adapter: llm.LLMLibAdapter) -> dict:
    """Dump adapter to dict."""
    return {
        "type": adapter.name(),
        "config": adapter.get_config()
    }


__all__ = [
    'adapters',
    'llm_adapter',
    'load_adapter'
]
