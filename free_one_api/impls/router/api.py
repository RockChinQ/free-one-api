import json
import traceback

import quart

from ...models.router import group as routergroup
from ...models.database import db
from ...models.channel import mgr as channelmgr
from ...models.key import mgr as apikeymgr
from ...entities import channel, apikey
from ...models import adapter


class WebAPIGroup(routergroup.APIGroup):
    
    chanmgr: channelmgr.AbsChannelManager
    
    keymgr: apikeymgr.AbsAPIKeyManager
    
    def __init__(self, dbmgr: db.DatabaseInterface, chanmgr: channelmgr.AbsChannelManager, keymgr: apikeymgr.AbsAPIKeyManager):
        super().__init__(dbmgr)
        self.chanmgr = chanmgr
        self.keymgr = keymgr
        self.group_name = "/api"
        
        @self.api("/channel/list", ["GET"])
        async def channel_list():
            # load channels from db to memory
            chan_list = await self.chanmgr.list_channels()
            
            chan_list_json = [channel.Channel.dump_channel(chan) for chan in chan_list]
            chan_list_json = [{
                "id": chan["id"],
                "name": chan["name"],
                "adapter": chan["adapter"]['type'],
                "enabled": chan["enabled"],
                "latency": chan["latency"],
            } for chan in chan_list_json]
            
            return quart.jsonify({
                "code": 0,
                "message": "ok",
                "data": chan_list_json,
            })
        
        @self.api("/channel/create", ["POST"])
        async def channel_create():
            data = await quart.request.get_json()
            
            chan = channel.Channel.load_channel(data)
            
            await self.chanmgr.create_channel(chan)
            
            return quart.jsonify({
                "code": 0,
                "message": "ok",
            })
            
        @self.api("/channel/delete/<int:chan_id>", ["DELETE"])
        async def channel_delete(chan_id: int):
            try:
                await self.chanmgr.delete_channel(chan_id)
                
                return quart.jsonify({
                    "code": 0,
                    "message": "ok",
                })
            except Exception as e:
                return quart.jsonify({
                    "code": 1,
                    "message": str(e),
                })
            
        @self.api("/channel/details/<int:chan_id>", ["GET"])
        async def channel_details(chan_id: int):
            try:
                chan = await self.chanmgr.get_channel(chan_id)
                
                return quart.jsonify({
                    "code": 0,
                    "message": "ok",
                    "data": channel.Channel.dump_channel(chan),
                })
            except Exception as e:
                return quart.jsonify({
                    "code": 1,
                    "message": str(e),
                })
            
        @self.api("/channel/update/<int:chan_id>", ["PUT"])
        async def channel_update(chan_id: int):
            try:
                chan = channel.Channel.load_channel(await quart.request.get_json())
                chan.id = chan_id
                await self.chanmgr.update_channel(chan)
                
                return quart.jsonify({
                    "code": 0,
                    "message": "ok",
                })
            except Exception as e:
                return quart.jsonify({
                    "code": 1,
                    "message": str(e),
                })
            
        @self.api("/channel/enable/<int:chan_id>", ["POST"])
        async def channel_enable(chan_id: int):
            try:
                await self.chanmgr.enable_channel(chan_id)
                
                return quart.jsonify({
                    "code": 0,
                    "message": "ok",
                })
            except Exception as e:
                return quart.jsonify({
                    "code": 1,
                    "message": str(e),
                })
            
        @self.api("/channel/disable/<int:chan_id>", ["POST"])
        async def channel_disable(chan_id: int):
            try:
                await self.chanmgr.disable_channel(chan_id)
                
                return quart.jsonify({
                    "code": 0,
                    "message": "ok",
                })
            except Exception as e:
                return quart.jsonify({
                    "code": 1,
                    "message": str(e),
                })
            
        @self.api("/channel/test/<int:chan_id>", ["POST"])
        async def channel_test(chan_id: int):
            try:
                latency = await self.chanmgr.test_channel(chan_id)
                
                return quart.jsonify({
                    "code": 0,
                    "message": "ok",
                    "data": {
                        "latency": latency,
                    },
                })
            except Exception as e:
                traceback.print_exc()
                return quart.jsonify({
                    "code": 1,
                    "message": str(e),
                })
            
        @self.api("/adapter/list", ["GET"])
        async def adapter_list():
            res = {
                "code": 0,
                "message": "ok",
                "data": adapter.list_adapters(),
            }
            
            return quart.jsonify(res)
            
        @self.api("/key/list", ["GET"])
        async def key_list():
            try:
                key_list = await self.keymgr.list_keys()
                
                key_list_json = []
                
                for key in key_list:
                    key_list_json.append({
                        "id": key.id,
                        "name": key.name,
                        "brief": key.raw[:10] + "..." + key.raw[-10:],
                        "created_at": key.created_at,
                    })
                
                return quart.jsonify({
                    "code": 0,
                    "message": "ok",
                    "data": key_list_json,
                })
            except Exception as e:
                return quart.jsonify({
                    "code": 1,
                    "message": str(e),
                })
            
        @self.api("/key/raw/<int:key_id>", ["GET"])
        async def key_raw(key_id: int):
            try:
                key = await self.keymgr.get_key(key_id)
                
                return quart.jsonify({
                    "code": 0,
                    "message": "ok",
                    "data": {
                        "key": key.raw,
                    },
                })
            except Exception as e:
                return quart.jsonify({
                    "code": 1,
                    "message": str(e),
                })
            
        @self.api("/key/create", ["POST"])
        async def key_create():
            try:
                data = await quart.request.get_json()
                
                key_name = data["name"]
                
                if await self.keymgr.has_key_name(key_name):
                    raise Exception("key name already exists: "+key_name)
                
                key = apikey.FreeOneAPIKey.make_new(key_name)
                
                await self.keymgr.create_key(key)
                
                return quart.jsonify({
                    "code": 0,
                    "message": "ok",
                    "data": {
                        "id": key.id,
                        "raw": key.raw,
                    },
                })
            except Exception as e:
                return quart.jsonify({
                    "code": 1,
                    "message": str(e),
                })

        @self.api("/key/revoke/<int:key_id>", ["DELETE"])
        async def key_revoke(key_id: int):
            try:
                await self.keymgr.revoke_key(key_id)
                
                return quart.jsonify({
                    "code": 0,
                    "message": "ok",
                })
            except Exception as e:
                import traceback
                traceback.print_exc()
                return quart.jsonify({
                    "code": 1,
                    "message": str(e),
                })