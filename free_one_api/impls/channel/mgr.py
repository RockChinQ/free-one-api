"""Channel management."""
import time

from ...entities import channel
from ...models.database import db
from ...models.channel import mgr


class ChannelManager(mgr.AbsChannelManager):
    """Channel manager.
    
    Provides channel creation, deletion, updating and listing,
    or selecting channel according to load balance algorithm.
    """

    def __init__(
        self,
        dbmgr: db.DatabaseInterface,
    ):
        self.dbmgr = dbmgr
        self.channels = []
        
    async def has_channel(self, channel_id: int) -> bool:
        for chan in self.channels:
            if chan.id == channel_id:
                return True
        return False
    
    async def has_channel_in_db(self, channel_id: int) -> bool:
        for chan in await self.dbmgr.list_channels():
            if chan.id == channel_id:
                return True
        return False
    
    async def get_channel(self, channel_id: int) -> channel.Channel:
        """Get a channel."""
        for chan in self.channels:
            if chan.id == channel_id:
                return chan
        raise ValueError("Channel not found.")

    async def list_channels(self) -> list[channel.Channel]:
        """List all channels."""
        self.channels = await self.dbmgr.list_channels()
        
        return self.channels
    
    async def create_channel(self, chan: channel.Channel) -> None:
        """Create a channel."""
        assert not await self.has_channel(chan.id)
        
        await self.dbmgr.insert_channel(chan)
        self.channels.append(chan)

    async def delete_channel(self, channel_id: int) -> None:
        """Delete a channel."""
        assert await self.has_channel(channel_id)
        
        await self.dbmgr.delete_channel(channel_id)
        for i in range(len(self.channels)):
            if self.channels[i].id == channel_id:
                del self.channels[i]
                break

    async def update_channel(self, chan: channel.Channel) -> None:
        """Update a channel."""
        assert await self.has_channel(chan.id)
        
        await self.dbmgr.update_channel(chan)
        for i in range(len(self.channels)):
            if self.channels[i].id == chan.id:
                self.channels[i] = chan
                break
            
    async def enable_channel(self, channel_id: int) -> None:
        """Enable a channel."""
        assert await self.has_channel(channel_id)
        
        chan = await self.get_channel(channel_id)
        chan.enabled = True
        await self.update_channel(chan)
        
    async def disable_channel(self, channel_id: int) -> None:
        """Disable a channel."""
        assert await self.has_channel(channel_id)
        
        chan = await self.get_channel(channel_id)
        chan.enabled = False
        await self.update_channel(chan)
        
    async def test_channel(self, channel_id: int) -> int:
        assert await self.has_channel(channel_id)
        
        chan = await self.get_channel(channel_id)
        # 计时
        now = time.time()
        latency = -1
        try:
            await chan.adapter.test()
        except Exception as e:
            raise ValueError("Test failed.") from e
        latency = time.time() - now
        
        chan.latency = latency
        await self.update_channel(chan)
        return latency

    async def select_channel(self) -> channel.Channel:
        """Select a channel.
        
        Implement load balance algorithm here.
        """
        # TODO: implement load balance algorithm