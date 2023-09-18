import datetime

from ..common import key


class FreeOneAPIKey:
    """API key."""
    id: int

    name: str
    """Name of this API key."""

    created_at: int
    """Created at."""

    raw: str
    """API key."""

    def __init__(self, id: str, name: str, created_at: int, raw: str):
        self.id = id
        self.name = name
        self.created_at = created_at
        self.raw = raw

    @classmethod
    def make_new(cls, name: str) -> 'FreeOneAPIKey':
        return cls(
            -1,
            name=name,
            created_at=datetime.datetime.now().timestamp(),
            raw=key.generate_api_key(),
        )
