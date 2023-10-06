import logging
import asyncio

from ..models.database import db


class SQLiteHandler(logging.Handler):
    """SQLite logging handler."""

    dbmgr: db.DatabaseInterface

    def __init__(self, dbmgr: db.DatabaseInterface, level=logging.NOTSET):
        super().__init__(level)
        self.dbmgr = dbmgr

    def emit(self, record: logging.LogRecord):
        """Emit a record."""
        loop = asyncio.get_running_loop()
        
        loop.create_task(self.dbmgr.insert_log(record.created, record.getMessage()))
