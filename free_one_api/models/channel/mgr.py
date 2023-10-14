import abc

import tiktoken

from ...entities import channel
from ..database import db
from ...entities import request, response


class AbsChannelManager(metaclass=abc.ABCMeta):
    """Base class for channel manager."""
    
    dbmgr: db.DatabaseInterface
    """Database manager."""

    channels: list[channel.Channel]
    """Channel list in runtime."""

    @abc.abstractmethod
    async def list_channels(self) -> list[channel.Channel]:
        """List all channels."""
        pass
    
    @abc.abstractmethod
    async def load_channels(self) -> None:
        """Load all channels from database."""
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
    async def select_channel(
        self,
        path: str,
        req: request.Request,
        id_suffix: str,
    ) -> channel.Channel:
        """Select a channel.
        
        Implement load balance algorithm here.
        
        Args:
            path: path of this request.
            req: request object.
            id_suffix: suffix of channel id.
        """
        pass
