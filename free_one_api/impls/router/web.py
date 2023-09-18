import os

import quart

from ...models.router import group as routergroup


class WebPageGroup(routergroup.APIGroup):
    
    frontend_path: str
    
    def __init__(self, config: dict):
        super().__init__(None)
        self.frontend_path = config["frontend_path"] if "frontend_path" in config else "./web/dist/"
        self.group_name = ""
        
        @self.api("/ping", ["GET"])
        async def ping():
            return "pong"
        
        @self.api("/", ["GET"])
        async def index():
            return await quart.send_from_directory(self.frontend_path, "index.html")
        
        @self.api("/<path:path>", ["GET"])
        async def static_proxy(path):
            if not os.path.exists(os.path.join(self.frontend_path, path)):
                return await quart.send_from_directory(self.frontend_path, "index.html")
            return await quart.send_from_directory(self.frontend_path, path)
