import abc

from ...entities import channel
from ..database import db


class AbsChannelManager(metaclass=abc.ABCMeta):
    
    dbmgr: db.DatabaseInterface
    """Database manager."""
    
    channels: list[channel.Channel]
    """Channel list in runtime."""

    @abc.abstractmethod
    async def list_channels(self) -> list[channel.Channel]:
        """List all channels."""
        pass
    
    @abc.abstractmethod
    async def create_channel(self, chan: channel.Channel) -> None:
        """Create a channel."""
        pass
    
    @abc.abstractmethod
    async def delete_channel(self, channel_id: int) -> None:
        """Delete a channel."""
        pass
    
    @abc.abstractmethod
    async def update_channel(self, chan: channel.Channel) -> None:
        """Update a channel."""
        pass
    
    @abc.abstractmethod
    async def enable_channel(self, channel_id: int) -> None:
        """Enable a channel."""
        pass
    
    @abc.abstractmethod
    async def disable_channel(self, channel_id: int) -> None:
        """Disable a channel."""
        pass
    
    @abc.abstractmethod
    async def test_channel(self, channel_id: int) -> int:
        """Test a channel.
        
        Return latency.
        """
        pass
    
    @abc.abstractmethod
    async def select_channel(self) -> channel.Channel:
        """Select a channel.
        
        Implement load balance algorithm here.
        """
        pass
    