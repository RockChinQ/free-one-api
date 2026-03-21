import os
import sys
import asyncio
import logging
import colorlog

import yaml

from ..common import crypto
from .router import mgr as routermgr
from .router import api as apigroup

from ..models.database import db
from ..models.channel import mgr as chanmgr
from ..models.key import mgr as keymgr
from ..models.router import group as routergroup
from ..models.watchdog import wd as wdmgr

from .adapter import revChatGPT
from .adapter import claude
from .adapter import bard
from .adapter import gpt4free
from .adapter import hugchat
from .adapter import qianwen
from .adapter import tiangong
from .adapter import kimi
from .adapter import re_gpt

from . import log
from . import cfg as cfgutil


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

    watchdog: wdmgr.AbsWatchDog

    logging_level: int = logging.INFO

    def __init__(
            self,
            dbmgr: db.DatabaseInterface,
            router: routermgr.RouterManager,
            channel: chanmgr.AbsChannelManager,
            key: keymgr.AbsAPIKeyManager,
            watchdog: wdmgr.AbsWatchDog,
            logging_level: int = logging.INFO,
    ):
        self.dbmgr = dbmgr
        self.router = router
        self.channel = channel
        self.key = key
        self.watchdog = watchdog
        self.logging_level = logging_level

    async def run(self):
        """Run application."""
        loop = asyncio.get_running_loop()

        loop.create_task(self.watchdog.run())

        await self.router.serve(loop)


log_colors_config = {
    'DEBUG': 'green',  # cyan white
    'INFO': 'white',
    'WARNING': 'yellow',
    'ERROR': 'red',
    'CRITICAL': 'cyan',
}

default_config = {
    "1-documentation": "see at https://github.com/RockChinQ/free-one-api",
    "database": {
        "type": "sqlite",
        "path": "./data/free_one_api.db",
    },
    "watchdog": {
        "heartbeat": {
            "interval": 1800,
            "timeout": 300,
            "fail_limit": 3,
        },
    },
    "router": {
        "port": 3000,
        "token": "12345678",
    },
    "web": {
        "frontend_path": "./web/dist/",
    },
    "logging": {
        "debug": False,
    },
    "random_ad": {
        "enabled": False,
        "rate": 0.05,
        "ad_list": [
            " (This response is sponsored by Free One API. Consider star the project on GitHub: https://github.com/RockChinQ/free-one-api )",
        ]
    },
    "adapters": {
        "acheong08_ChatGPT": {
            "reverse_proxy": "https://chatproxy.rockchin.top/api/",
            "auto_ignore_duplicated": True,
        }
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

    # complete config
    config = cfgutil.complete_config(config, default_config)

    # dump config
    with open(config_path, "w") as f:
        yaml.dump(config, f)

    # logging
    logging_level = logging.INFO

    if 'logging' in config and 'debug' in config['logging'] and config['logging']['debug']:
        logging_level = logging.DEBUG

    if 'DEBUG' in os.environ and os.environ['DEBUG'] == 'true':
        logging_level = logging.DEBUG

    print("Logging level:", logging_level)
    logging.debug("Debug mode enabled.")

    terminal_out = logging.StreamHandler()

    terminal_out.setLevel(logging_level)
    terminal_out.setFormatter(colorlog.ColoredFormatter(
        "[%(asctime)s.%(msecs)03d] %(log_color)s%(pathname)s (%(lineno)d) - [%(levelname)s] :\n"
        "%(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
        log_colors=log_colors_config,
    ))

    for handler in logging.getLogger().handlers:
        logging.getLogger().removeHandler(handler)

    logging.getLogger().addHandler(terminal_out)

    # save ad to runtime
    if 'random_ad' in config and config['random_ad']['enabled']:
        from ..common import randomad

        randomad.enabled = config['random_ad']['enabled']
        randomad.rate = config['random_ad']['rate']
        randomad.ads = config['random_ad']['ad_list']

    from ..common import randomad

    # make database manager
    from .database import sqlite as sqlitedb

    dbmgr_cls_mapping = {
        "sqlite": sqlitedb.SQLiteDB,
    }

    dbmgr = dbmgr_cls_mapping[config['database']['type']](config['database'])
    await dbmgr.initialize()

    # database handler
    dblogger = log.SQLiteHandler(dbmgr)

    # failed to set debug level for db handler
    dblogger.setLevel(logging.INFO if logging_level <= logging.INFO else logging_level)
    dblogger.setFormatter(
        logging.Formatter("[%(asctime)s.%(msecs)03d] %(pathname)s (%(lineno)d) - [%(levelname)s] :\n%(message)s"))

    logging.getLogger().addHandler(dblogger)

    # set default values
    # apply adapters config
    if 'misc' in config and 'chatgpt_api_base' in config['misc']:  # backward compatibility
        config['adapters']['acheong08_ChatGPT']['reverse_proxy'] = config['misc']['chatgpt_api_base']

    adapter_config_mapping = {
        "acheong08_ChatGPT": revChatGPT.RevChatGPTAdapter,
        "KoushikNavuluri_Claude-API": claude.ClaudeAdapter,
        "dsdanielpark_Bard-API": bard.BardAdapter,
        "xtekky_gpt4free": gpt4free.GPT4FreeAdapter,
        "Soulter_hugging-chat-api": hugchat.HuggingChatAdapter,
        "xw5xr6_revTongYi": qianwen.QianWenAdapter,
        "DrTang": tiangong.tiangong,
        "DrTang": kimi.KimiAdapter,
        "Zai-Kun_reverse-engineered-chatgpt": re_gpt.ReGPTAdapter,
    }

    for adapter_name in adapter_config_mapping:
        if adapter_name not in config['adapters']:
            config['adapters'][adapter_name] = {}

        for k, v in config["adapters"][adapter_name].items():
            setattr(adapter_config_mapping[adapter_name], k, v)

    # make channel manager
    from .channel import mgr as chanmgr

    channelmgr = chanmgr.ChannelManager(dbmgr)
    await channelmgr.load_channels()

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

    # watchdog and tasks
    from .watchdog import wd as watchdog

    wdmgr = watchdog.WatchDog()

    # tasks
    from .watchdog.tasks import heartbeat

    hbtask = heartbeat.HeartBeatTask(
        channelmgr,
        config['watchdog']['heartbeat'],
    )

    wdmgr.add_task(hbtask)

    app = Application(
        dbmgr=dbmgr,
        router=routermgr,
        channel=channelmgr,
        key=apikeymgr,
        watchdog=wdmgr,
        logging_level=logging_level,
    )

    logging.info("Application initialized.")

    return app
