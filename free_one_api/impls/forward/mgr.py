import time
import json
import string
import random

import quart

from ...models.forward import mgr as forwardmgr
from ...models.channel import mgr as channelmgr
from ...models.key import mgr as apikeymgr
from ...entities import channel, apikey, request, response


class ForwardManager(forwardmgr.AbsForwardManager):
    
    def __init__(self, chanmgr: channelmgr.AbsChannelManager, keymgr: apikeymgr.AbsAPIKeyManager):
        self.chanmgr = chanmgr
        self.keymgr = keymgr
        
    async def __stream_query(
        self,
        chan: channel.Channel,
        req: request.Request,
    ) -> quart.Response:
        before = time.time()
        id_suffix = "".join(random.choices(string.ascii_letters+string.digits, k=29))
        
        t = int(time.time())
        async def _gen():
            async for resp in chan.adapter.query(req):
                
                if (resp.normal_message is None or len(resp.normal_message) == 0) and resp.finish_reason == response.FinishReason.NULL:
                    continue
                
                yield "data: {}\n\n".format(json.dumps({
                    "id": "chatcmpl-"+id_suffix,
                    "object": "chat.completion.chunk",
                    "created": t,
                    "model": req.model,
                    "choices": [{
                        "index": 0,
                        "delta": {
                            "content": resp.normal_message,
                        } if resp.normal_message else {},
                        "finish_reason": resp.finish_reason.value
                    }]
                }))
            yield "data: [DONE]\n\n"
        
        spent_ms = int((time.time() - before)*1000)
        
        headers = {
            "Content-Type": "text/event-stream",
            "Transfer-Encoding": "chunked",
            "Connection": "keep-alive",
            "openai-processing-ms": str(spent_ms),
            "openai-version": "2020-10-01",
            "Cache-Control": "no-cache",
            "X-Accel-Buffering": "no",
        }
        
        return quart.Response(
            _gen(),
            mimetype="text/event-stream",
            headers=headers,
        )
    
    async def __non_stream_query(
        self,
        chan: channel.Channel,
        req: request.Request,
    ) -> quart.Response:
        before = time.time()
        id_suffix = "".join(random.choices(string.ascii_letters+string.digits, k=29))
        
        normal_message = ""
        
        resp_tmp: response.Response = None
        
        async for resp in chan.adapter.query(req):
            if resp.normal_message is not None:
                resp_tmp = resp
                normal_message += resp.normal_message
                
        spent_ms = int((time.time() - before)*1000)
        
        prompt_tokens = chan.count_tokens(req.model, req.messages)
        completion_tokens = chan.count_tokens(
            req.model,
            [{
                "role": "assistant",
                "content": normal_message,
            }]
        )
        
        result = {
            "id": "chatcmpl-"+id_suffix,
            "object": "chat.completion",
            "created": int(time.time()),
            "model": req.model,
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": normal_message,
                    },
                    "finish_reason": resp_tmp.finish_reason.value
                }
            ],
            "usage": {
                "prompt_tokens": prompt_tokens,
                "completion_tokens": completion_tokens,
                "total_tokens": prompt_tokens + completion_tokens,
            }
        }
        
        return quart.jsonify(result)

    async def query(
        self,
        path: str,
        req: request.Request,
        raw_data: dict,
    ) -> quart.Response:
        
        chan: channel.Channel = await self.chanmgr.select_channel(
            path,
            req,
        )
        
        if chan is None:
            pass
        
        if req.stream:
            return await self.__stream_query(chan, req)
        else:
            return await self.__non_stream_query(chan, req)