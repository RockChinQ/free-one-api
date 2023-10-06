import abc

from ...entities import channel, apikey


class DatabaseInterface(metaclass=abc.ABCMeta):
    """Base class for database interface."""

    @abc.abstractmethod
    async def get_channel(self, channel_id: int) -> channel.Channel:
        """Get a channel."""
        return

    @abc.abstractmethod
    async def list_channels(self) -> list[channel.Channel]:
        """Load all channels."""
        return

    @abc.abstractmethod
    async def insert_channel(self, chan: channel.Channel) -> None:
        """Insert a channel. Set the id of the channel."""
        return

    @abc.abstractmethod
    async def update_channel(self, chan: channel.Channel) -> None:
        """Update a channel."""
        return

    @abc.abstractmethod
    async def delete_channel(self, channel_id: int) -> None:
        """Delete a channel."""
        return

    @abc.abstractmethod
    async def list_keys(self) -> list[apikey.FreeOneAPIKey]:
        """Load all keys."""
        return

    @abc.abstractmethod
    async def insert_key(self, key: apikey.FreeOneAPIKey) -> None:
        """Insert a key. Set the id of the key."""
        return

    @abc.abstractmethod
    async def update_key(self, key: apikey.FreeOneAPIKey) -> None:
        """Update a key."""
        return

    @abc.abstractmethod
    async def delete_key(self, key_id: int) -> None:
        """Delete a key."""
        return

    @abc.abstractmethod
    async def insert_log(self, timestamp: int, content: str) -> None:
        """Insert a log."""
        return

    @abc.abstractmethod
    async def select_logs(self, time_range: tuple[int, int]) -> list[tuple[int, str]]:
        """Select logs.
        
        Args:
            time_range: (start, end)
        """
        return
