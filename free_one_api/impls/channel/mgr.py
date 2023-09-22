"""Channel management."""
import time
import asyncio
import random

from ...entities import channel, request
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
            res, error = await chan.adapter.test()
            if not res:
                raise ValueError(error)
        except Exception as e:
            raise ValueError("Test failed.") from e
        latency = int((time.time() - now)*100)/100
        
        chan.latency = latency
        await self.update_channel(chan)
        return latency

    async def select_channel(
        self,
        path: str,
        req: request.Request,
    ) -> channel.Channel:
        """Select a channel.
        
        Method here will filter channels and select the best one.
        
        Hard filters, which channel not match these conditions will be excluded:
        1. disabled channels.
        2. path the client request.
        3. model name the client request.
        
        Soft filters, these filter give score to each channel,
        the channel with the highest score will be selected:
        1. support for stream mode matching the client request.
        2. support for multi-round matching the client request.
        3. support for function calling.
        4. usage times in lifetime.
        
        Args:
            path: path of this request.
            req: request object.
            
        """
        stream_mode = req.stream
        has_functions = req.functions is not None and len(req.functions) > 0
        is_multi_round = req.messages is not None and len(req.messages) > 0
        
        model_name = req.model
        
        channel_copy = self.channels.copy()
        
        # delete disabled channels
        channel_copy = list(filter(lambda chan: chan.enabled, channel_copy))
        
        # delete not matched path
        channel_copy = list(filter(lambda chan: chan.adapter.supported_path() == path, channel_copy))
        
        # delete not matched model name
        channel_copy_tmp = []
        
        for chan in channel_copy:
            models = []
            left_model_names = list(chan.model_mapping.keys())
            models.extend(left_model_names)
            models.extend(chan.adapter.supported_models())
            
            if model_name in models:
                channel_copy_tmp.append(chan)
                
        channel_copy = channel_copy_tmp
        
        # i just randomly select one channel now!
        random.seed(time.time())
        return random.choice(channel_copy)
        