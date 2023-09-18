import asyncio

from ...entities import apikey
from ...models.database import db
from ...models.key import mgr as keymgr


class APIKeyManager(keymgr.AbsAPIKeyManager):
    """API Key manager."""
    
    def __init__(self, dbmgr: db.DatabaseInterface):
        self.dbmgr = dbmgr
        self.keys = []
        
    async def has_key(self, key_id: int) -> bool:
        for key in self.keys:
            if key.id == key_id:
                return True
        return False
    
    async def has_key_in_db(self, key_id: int) -> bool:
        for key in await self.dbmgr.list_keys():
            if key.id == key_id:
                return True
        return False
    
    async def has_key_name(self, key_name: str) -> bool:
        for key in self.keys:
            if key.name == key_name:
                return True
        return False
    
    async def has_key_name_in_db(self, key_name: str) -> bool:
        for key in await self.dbmgr.list_keys():
            if key.name == key_name:
                return True
        return False
    
    async def list_keys(self) -> list[apikey.FreeOneAPIKey]:
        """List all keys."""
        self.keys = await self.dbmgr.list_keys()
        
        return self.keys
    
    async def create_key(self, key: apikey.FreeOneAPIKey) -> None:
        # key already created by upper caller
        # only insert to db and save to memory here
        await self.dbmgr.insert_key(key)
        self.keys.append(key)
        
    async def revoke_key(self, key_id: int) -> None:
        assert await self.has_key(key_id)
        
        await self.dbmgr.delete_key(key_id)
        for i in range(len(self.keys)):
            if self.keys[i].id == key_id:
                del self.keys[i]
                break
            
    async def get_key(self, key_id: int) -> apikey.FreeOneAPIKey:
        """Get a key."""
        for key in self.keys:
            if key.id == key_id:
                return key
        raise ValueError("Key not found.")
