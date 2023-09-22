import quart

from ...models.router import group as routergroup
from ...models.key import mgr as apikeymgr
from ...models.channel import mgr as channelmgr
from ...models.database import db
from ...models.forward import mgr as forwardmgr
from ...entities import channel, apikey, request, response


class ForwardAPIGroup(routergroup.APIGroup):
    """Forward API group."""
    
    chanmgr: channelmgr.AbsChannelManager
    """Channel manager."""
    
    keymgr: apikeymgr.AbsAPIKeyManager
    """API key manager."""
    
    fwdmgr: forwardmgr.AbsForwardManager
    """Forward manager."""
    
    def __init__(
        self,
        dbmgr: db.DatabaseInterface,
        chanmgr: channelmgr.AbsChannelManager,
        keymgr: apikeymgr.AbsAPIKeyManager,
        fwdmgr: forwardmgr.AbsForwardManager,
    ):
        super().__init__(dbmgr)
        self.forwardmgr = forwardmgr
        self.group_name = ""
        self.chanmgr = chanmgr
        self.keymgr = keymgr
        self.fwdmgr = fwdmgr

        @self.api("/v1/chat/completions", ["POST"], auth=True)
        async def chat_completion():
            """Chat completion."""
            
            raw_data = await quart.request.get_json()
            
            req = request.Request(
                raw_data["model"],
                raw_data["messages"],
                raw_data["functions"] if "functions" in raw_data else None,
                'stream' in raw_data and raw_data["stream"],
            )
            
            return await self.fwdmgr.query(
                "/v1/chat/completions",
                req,
                raw_data,
            )
        
    def get_tokens(self) -> list[str]:
        """Override to use keys in keymgr as tokens."""
        key_obj_list: apikey.FreeOneAPIKey = self.keymgr.get_key_list()
        
        key_list = [key_obj.raw for key_obj in key_obj_list]
        
        return key_list
