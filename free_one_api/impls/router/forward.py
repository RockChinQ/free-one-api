import traceback

import quart

from ...models.router import group as routergroup
from ...models.key import mgr as apikeymgr
from ...models.channel import mgr as channelmgr
from ...models.database import db
from ...models.forward import mgr as forwardmgr
from ...entities import channel, apikey, request, response, exceptions
from ..forward import protocol as protocolmod


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
            try:
            
                raw_data = await quart.request.get_json()
                
                req = request.Request(
                    raw_data["model"],
                    raw_data["messages"],
                    raw_data["functions"] if "functions" in raw_data else None,
                    'stream' in raw_data and raw_data["stream"],
                    raw_data.get("tools"),
                    raw_data.get("tool_choice"),
                )
                
                return await self.fwdmgr.query(
                    "/v1/chat/completions",
                    req,
                    raw_data,
                )
            except exceptions.QueryHandlingError as e:
                return quart.jsonify(
                    {
                        "error": {
                            "message": e.message,
                            "type": e.type,
                            "param": e.param,
                            "code": e.code
                        }
                    }
                ), e.status_code
            except Exception as e:
                traceback.print_exc()
                return quart.jsonify(
                    {
                        "error": {
                            "message": "Error occurred while handling your request: {}. You can retry or contact your admin. See the documentation for details at https://github.com/RockChinQ/free-one-api".format(str(e)),
                            "type": "requests",
                            "param": None,
                            "code": None
                        }
                    }
                ), 500
        
        @self.api("/v1/models", ["GET"], auth=True)
        async def list_models():
            """List available models (OpenAI compatible).

            模型名来自已启用渠道:model_mapping 的键(客户端请求名)与适配器
            supported_models()。不要在适配器旁再维护一份写死的清单。
            """
            models: list[str] = []
            for chan in await self.chanmgr.list_channels():
                if not chan.enabled:
                    continue
                if chan.adapter.supported_path() != "/v1/chat/completions":
                    continue
                for model in list(chan.model_mapping.keys()) + list(chan.adapter.supported_models()):
                    if model and model not in models:
                        models.append(model)
            models.sort()
            return quart.jsonify({
                "object": "list",
                "data": [
                    {
                        "id": m, "object": "model", "created": 0, "owned_by": "free-one-api",
                    } for m in models
                ],
            })
        @self.api("/v1/responses", ["POST"], auth=True)
        async def responses_api():
            # OpenAI Responses API,内部转成 Chat Completions 走同一条适配器链路。
            try:
                raw_data = await quart.request.get_json()
                parsed = protocolmod.parse_responses_request(raw_data)
                req = request.Request(
                    parsed["model"],
                    parsed["messages"],
                    None,
                    parsed["stream"],
                    parsed["tools"],
                    parsed["tool_choice"],
                )
                return await self.fwdmgr.query(
                    "/v1/responses",
                    req,
                    raw_data,
                    proto=protocolmod.PROTOCOL_RESPONSES,
                )
            except exceptions.QueryHandlingError as e:
                body, status = protocolmod.error_payload(
                    protocolmod.PROTOCOL_RESPONSES, e.status_code, e.code, e.message, e.type
                )
                return quart.jsonify(body), status
            except Exception as e:
                traceback.print_exc()
                body, status = protocolmod.error_payload(
                    protocolmod.PROTOCOL_RESPONSES, 500, None, str(e)
                )
                return quart.jsonify(body), status

        @self.api("/v1/messages", ["POST"], auth=True)
        async def anthropic_messages():
            # Anthropic Messages API,同上。
            try:
                raw_data = await quart.request.get_json()
                parsed = protocolmod.parse_anthropic_request(raw_data)
                req = request.Request(
                    parsed["model"],
                    parsed["messages"],
                    None,
                    parsed["stream"],
                    parsed["tools"],
                    parsed["tool_choice"],
                )
                return await self.fwdmgr.query(
                    "/v1/messages",
                    req,
                    raw_data,
                    proto=protocolmod.PROTOCOL_ANTHROPIC,
                )
            except exceptions.QueryHandlingError as e:
                body, status = protocolmod.error_payload(
                    protocolmod.PROTOCOL_ANTHROPIC, e.status_code, e.code, e.message, e.type
                )
                return quart.jsonify(body), status
            except Exception as e:
                traceback.print_exc()
                body, status = protocolmod.error_payload(
                    protocolmod.PROTOCOL_ANTHROPIC, 500, None, str(e)
                )
                return quart.jsonify(body), status
    def get_tokens(self) -> list[str]:
        """Override to use keys in keymgr as tokens."""
        key_obj_list: apikey.FreeOneAPIKey = self.keymgr.get_key_list()
        
        key_list = [key_obj.raw for key_obj in key_obj_list]
        
        return key_list

    def check_auth(self, auth: str) -> quart.Response:
        if auth is None:
            alt = quart.request.headers.get("x-api-key") or quart.request.headers.get("api-key")
            if alt:
                auth = "Bearer " + alt
        if auth is None or not auth.startswith("Bearer "):
            return quart.jsonify(
                {
                    "error": {
                        "message": "You didn't provide an API key. You need to provide your API key in an Authorization header using Bearer auth (i.e. Authorization: Bearer YOUR_KEY), or as the password field (with blank username) if you're accessing the API from your browser and are prompted for a username and password. You can obtain an API key from the free-one-api admin page, please contact the admin. See documentation for details at https://github.com/RockChinQ/free-one-api",
                        "type": "invalid_request_error",
                        "param": None,
                        "code": None
                    }
                }
            ), 401
        token = auth[7:]
        
        if token not in self.get_tokens():
            prefix = token[:8]
            suffix = token[-4:]
            mid = "*" * ((51 - len(prefix) - len(suffix)) if (len(prefix) + len(suffix) < 51) else 0)
            return quart.jsonify(
                {
                    "error": {
                        "message": "Incorrect API key provided: "+prefix+mid+suffix+". You can find you API key on the admin page of free-one-api, please contact the admin. See documentation for details at https://github.com/RockChinQ/free-one-api",
                        "type": "invalid_request_error",
                        "param": None,
                        "code": "invalid_api_key"
                    }
                }
            ), 401
            
        return None
