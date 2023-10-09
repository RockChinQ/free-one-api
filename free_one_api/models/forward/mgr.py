import abc

import quart

from ...models.channel import mgr as channelmgr
from ...models.key import mgr as apikeymgr
from ...entities import channel, apikey
from ...models import adapter
from ...entities import request, response


supported_paths = [
    "/v1/chat/completions",
]


class AbsForwardManager(metaclass=abc.ABCMeta):
    """Abstract forward manager.
    
    Receive request object, Select a channel from channel manager,
    call the channel adapter, receive response object, wrapper it 
    depends on the stream mode setting, and return structure.
    """
    
    chanmgr: channelmgr.AbsChannelManager
    """Channel manager."""
    
    keymgr: apikeymgr.AbsAPIKeyManager
    """API key manager."""
    
    @abc.abstractmethod
    async def query(
        self,
        path: str,
        req: request.Request,
        raw_data: dict,
    ) -> quart.Response:
        """Query.
        
        Args:
            path: path.
            req: request object.
            raw_data: raw structure data.
            stream_mode: stream mode.
        """
        pass
