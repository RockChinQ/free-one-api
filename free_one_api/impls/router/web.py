import os

import quart

from ...models.router import group as routergroup
from ...common import crypto


class WebPageGroup(routergroup.APIGroup):
    
    frontend_path: str
    
    def __init__(self, webcfg: dict, routercfg: dict):
        super().__init__(None)
        self.frontend_path = webcfg["frontend_path"] if "frontend_path" in webcfg else "./web/dist/"
        self.group_name = ""
        
        @self.api("/check_password", ["POST"])
        async def check_password():
            data = await quart.request.get_json()
            if "password" not in data:
                return quart.jsonify({
                    "code": 1,
                    "message": "No password provided."
                })
            if data["password"] != crypto.md5_digest(routercfg["token"]):
                return quart.jsonify({
                    "code": 2,
                    "message": "Wrong token."
                })
            return quart.jsonify({
                "code": 0,
                "message": "ok"
            })
        
        @self.api("/ping", ["GET"])
        async def ping():
            return "pong"
        
        @self.api("/", ["GET"])
        async def index():
            return await quart.send_from_directory(self.frontend_path, "index.html")
        
        @self.api("/<path:path>", ["GET"])
        async def static_proxy(path):
            # if not os.path.exists(os.path.join(self.frontend_path, path)):
            #     return await quart.send_from_directory(self.frontend_path, "index.html")
            return await quart.send_from_directory(self.frontend_path, path)
