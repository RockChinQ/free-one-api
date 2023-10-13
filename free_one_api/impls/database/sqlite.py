import os
import json

import aiosqlite

from ...models.database import db as dbmod
from ...models import adapter
from ...entities import channel, apikey
from ..channel import eval as evl

channel_table_sql = """
CREATE TABLE IF NOT EXISTS channel (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    adapter JSON NOT NULL,
    model_mapping JSON NOT NULL,
    enabled INTEGER NOT NULL,
    latency INTEGER NOT NULL
)
"""

key_table_sql = """
CREATE TABLE IF NOT EXISTS apikey (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    raw TEXT NOT NULL
)
"""

log_table_sql = """
CREATE TABLE IF NOT EXISTS log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,
    content TEXT NOT NULL
)
"""


class SQLiteDB(dbmod.DatabaseInterface):

    def __init__(self, config: dict):
        self.config = config
        self.db_path = config['path']

    async def initialize(self):
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute(channel_table_sql)
            await db.execute(key_table_sql)
            await db.execute(log_table_sql)
            await db.commit()

    async def list_channels(self) -> list[channel.Channel]:
        async with aiosqlite.connect(self.db_path) as db:
            async with db.execute("SELECT * FROM channel") as cursor:
                rows = await cursor.fetchall()
                return [channel.Channel(
                    id=row[0],
                    name=row[1],
                    adapter=adapter.load_adapter(json.loads(row[2]), eval),
                    model_mapping=json.loads(row[3]),
                    enabled=bool(row[4]),
                    latency=row[5],
                    eval=evl.ChannelEvaluation(),
                ) for row in rows]

    async def insert_channel(self, chan: channel.Channel) -> None:
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute("INSERT INTO channel (name, adapter, model_mapping, enabled, latency) VALUES (?, ?, ?, ?, ?)", (
                chan.name,
                json.dumps(adapter.dump_adapter(chan.adapter)),
                json.dumps(chan.model_mapping),
                int(chan.enabled),
                chan.latency,
            ))
            await db.commit()
            async with db.execute("SELECT last_insert_rowid()") as cursor:
                row = await cursor.fetchone()
                chan.id = row[0]
  
    async def update_channel(self, chan: channel.Channel) -> None:
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute("UPDATE channel SET name = ?, adapter = ?, model_mapping = ?, enabled = ?, latency = ? WHERE id = ?", (
                chan.name,
                json.dumps(adapter.dump_adapter(chan.adapter)),
                json.dumps(chan.model_mapping),
                int(chan.enabled),
                chan.latency,
                chan.id,
            ))
            await db.commit()

    async def delete_channel(self, channel_id: int) -> None:
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute("DELETE FROM channel WHERE id = ?", (channel_id,))
            await db.commit()

    async def list_keys(self) -> list[apikey.FreeOneAPIKey]:
        async with aiosqlite.connect(self.db_path) as db:
            async with db.execute("SELECT * FROM apikey") as cursor:
                rows = await cursor.fetchall()
                return [apikey.FreeOneAPIKey(
                    id=row[0],
                    name=row[1],
                    created_at=row[2],
                    raw=row[3],
                ) for row in rows]

    async def insert_key(self, key: apikey.FreeOneAPIKey) -> None:
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute("INSERT INTO apikey (name, created_at, raw) VALUES (?, ?, ?)", (
                key.name,
                key.created_at,
                key.raw,
            ))
            await db.commit()
            async with db.execute("SELECT last_insert_rowid()") as cursor:
                row = await cursor.fetchone()
                key.id = row[0]

    async def update_key(self, key: apikey.FreeOneAPIKey) -> None:
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute("UPDATE apikey SET name = ?, created_at = ?, raw = ? WHERE id = ?", (
                key.name,
                key.created_at,
                key.raw,
                key.id,
            ))
            await db.commit()

    async def delete_key(self, key_id: int) -> None:
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute("DELETE FROM apikey WHERE id = ?", (key_id,))
            await db.commit()

    async def insert_log(self, timestamp: int, content: str) -> None:
        async with aiosqlite.connect(self.db_path) as db:
            await db.execute("INSERT INTO log (timestamp, content) VALUES (?, ?)", (
                timestamp,
                content,
            ))
            await db.commit()

    async def select_logs(self, time_range: tuple[int, int]) -> list[tuple[int, int, str]]:
        async with aiosqlite.connect(self.db_path) as db:
            async with db.execute("SELECT * FROM log WHERE timestamp >= ? AND timestamp <= ?", time_range) as cursor:
                rows = await cursor.fetchall()
                return [(row[0], row[1], row[2]) for row in rows]

    async def select_logs_page(self, capacity: int, page: int) -> list[tuple[int, int, str]]:
        async with aiosqlite.connect(self.db_path) as db:
            async with db.execute("SELECT * FROM log ORDER BY id DESC LIMIT ? OFFSET ?", (capacity, capacity * page)) as cursor:
                rows = await cursor.fetchall()
                return [(row[0], row[1], row[2]) for row in rows]
    
    async def get_logs_amount(self) -> int:
        async with aiosqlite.connect(self.db_path) as db:
            async with db.execute("SELECT COUNT(*) FROM log") as cursor:
                row = await cursor.fetchone()
                return row[0]
    