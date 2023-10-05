import os

import quart


class RouterManager:
    """Router manager.
    
    Register all paths and serve.
    """

    port: int
    frontend_dir: str
    _app: quart.Quart

    def __init__(self, routes: list[tuple[str, list[str], callable, dict]], config: dict):
        self.port = config["port"] if "port" in config else 3000
        self._app = quart.Quart(__name__)

        for route, methods, handler, kwargs in routes:
            for method in methods:
                self._app.route(route, methods=[method], **kwargs)(handler)

    async def serve(self, loop):
        """Serve API."""
        return await self._app.run_task(host="0.0.0.0", port=self.port)


if __name__ == "__main__":
    # print workdir
    print(os.getcwd())

    frontend_dir = "../../../web/dist/"
    
    # register frontend apis
    async def ping():
        return "pong"
    
    # frontend page and public files
    async def index():
        return await quart.send_from_directory(frontend_dir, "index.html")
    
    async def static_proxy(path):
        if not os.path.exists(os.path.join(frontend_dir, path)):
            return await quart.send_from_directory(frontend_dir, "index.html")
        return await quart.send_from_directory(frontend_dir, path)

    api = RouterManager(
        routes=[
            ("/ping", ["GET"], ping, {}),
            ("/", ["GET"], index, {}),
            ("/<path:path>", ["GET"], static_proxy, {}),
        ],
        config={
            "port": 3000
        }
    )
    api.serve()
