import abc

from ...entities import apikey
from ..database import db

class AbsAPIKeyManager(metaclass=abc.ABCMeta):
    
    dbmgr: db.DatabaseInterface
    """Database manager."""
    
    keys: list[apikey.FreeOneAPIKey]
    """Key list in runtime."""
    
    @abc.abstractmethod
    async def has_key(self, key_id: int) -> bool:
        """Check if a key exists."""
        pass
    
    @abc.abstractmethod
    async def has_key_in_db(self, key_id: int) -> bool:
        """Check if a key exists in database."""
        pass
    
    @abc.abstractmethod
    async def has_key_name(self, key_name: str) -> bool:
        """Check if a key name exists."""
        pass
    
    @abc.abstractmethod
    async def has_key_name_in_db(self, key_name: str) -> bool:
        """Check if a key name exists in database."""
        pass
    
    @abc.abstractmethod
    async def list_keys(self) -> list[apikey.FreeOneAPIKey]:
        """List all keys."""
        pass
    
    @abc.abstractmethod
    async def create_key(self, key: apikey.FreeOneAPIKey) -> None:
        """Create a key."""
        pass
    
    @abc.abstractmethod
    async def revoke_key(self, key_id: int) -> None:
        """Revoke a key."""
        pass
    
    @abc.abstractmethod
    async def get_key(self, key_id: int) -> apikey.FreeOneAPIKey:
        """Get a key."""
        pass
