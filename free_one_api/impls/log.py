import logging
import asyncio

from ..models.database import db


class SQLiteHandler(logging.Handler):
    """SQLite logging handler."""

    dbmgr: db.DatabaseInterface
    
    queue: asyncio.Queue = None

    def __init__(self, dbmgr: db.DatabaseInterface, level=logging.NOTSET):
        super().__init__(level)
        self.dbmgr = dbmgr
        
    async def _consumer(self):
        while True:
            record: logging.LogRecord = await self.queue.get()
            await self.dbmgr.insert_log(record.created, record.getMessage())
            self.queue.task_done()

    def emit(self, record: logging.LogRecord):
        """Emit a record."""
        loop = asyncio.get_running_loop()
        
        if self.queue is None:
            self.queue = asyncio.Queue()
            loop.create_task(self._consumer())
            
        loop.create_task(self.queue.put(record))
