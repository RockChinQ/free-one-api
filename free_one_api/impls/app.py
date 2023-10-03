import os
import sys

import yaml

from ..common import crypto
from .router import mgr as routermgr
from .router import api as apigroup

from ..models.database import db
from ..models.channel import mgr as chanmgr
from ..models.key import mgr as keymgr
from ..models.router import group as routergroup

from .adapter import revChatGPT
from .adapter import claude
from .adapter import bard
from .adapter import gpt4free
from .adapter import hugchat
from .adapter import qianwen


class Application:
    """Application instance."""

    dbmgr: db.DatabaseInterface
    """Database manager."""""

    router: routermgr.RouterManager
    """Router manager."""

    channel: chanmgr.AbsChannelManager
    """Channel manager."""
    
    key: keymgr.AbsAPIKeyManager
    """API Key manager."""
    
    def __init__(
        self,
        dbmgr: db.DatabaseInterface,
        router: routermgr.RouterManager,
        channel: chanmgr.AbsChannelManager,
        key: keymgr.AbsAPIKeyManager,
    ):
        self.dbmgr = dbmgr
        self.router = router
        self.channel = channel
        self.key = key
        
    def run(self):
        """Run application."""
        return self.router.serve()

default_config = {
    "database": {
        "type": "sqlite",
        "path": "./data/free_one_api.db",
    },
    "channel": {
        "heartbeat": {
            "interval": 1800,
            "fail_limit": 3,
        },
    },
    "router": {
        "port": 3000,
        "token": "12345678",
    },
    "web": {
        "frontend_path": "./web/dist/",
    }
}

async def make_application(config_path: str) -> Application:
    """Make application."""
    if not os.path.exists(config_path):
        with open(config_path, "w") as f:
            yaml.dump(default_config, f)
            print("Config file created at", config_path)
            # print("Please edit it and run again.")
            # sys.exit(0)
    config = {}
    with open(config_path, "r") as f:
        config = yaml.load(f, Loader=yaml.FullLoader)
        
    # make database manager
    from .database import sqlite as sqlitedb
    
    dbmgr_cls_mapping = {
        "sqlite": sqlitedb.SQLiteDB,
    }
    
    dbmgr = dbmgr_cls_mapping[config['database']['type']](config['database'])
    await dbmgr.initialize()
    
    # make channel manager
    from .channel import mgr as chanmgr
    
    channelmgr = chanmgr.ChannelManager(dbmgr)
    await channelmgr.list_channels()
    
    # make key manager
    from .key import mgr as keymgr
    
    apikeymgr = keymgr.APIKeyManager(dbmgr)
    await apikeymgr.list_keys()
    
    # make forward manager
    from .forward import mgr as forwardmgr
    
    fwdmgr = forwardmgr.ForwardManager(channelmgr, apikeymgr)
    
    # make router manager
    from .router import mgr as routermgr
    
    #   import all api groups
    from .router import forward as forwardgroup
    from .router import api as apigroup
    from .router import web as webgroup
    
    # ========= API Groups =========
    group_forward = forwardgroup.ForwardAPIGroup(dbmgr, channelmgr, apikeymgr, fwdmgr)
    group_api = apigroup.WebAPIGroup(dbmgr, channelmgr, apikeymgr)
    group_api.tokens = [crypto.md5_digest(config['router']['token'])]
    group_web = webgroup.WebPageGroup(config['web'], config['router'])
    
    paths = []
    
    paths += group_forward.get_routers()
    paths += group_web.get_routers()
    paths += group_api.get_routers()
    
    # ========= API Groups =========
    
    routermgr = routermgr.RouterManager(
        routes=paths,
        config=config['router'],
    )
    
    app = Application(
        dbmgr=dbmgr,
        router=routermgr,
        channel=channelmgr,
        key=apikeymgr,
    )
    
    return app
    