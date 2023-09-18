import abc

from ..database import db


class APIGroup(metaclass=abc.ABCMeta):
    
    group_name: str
    """Name of this group, also the path prefix of all APIs in this group.
    
    e.g. /api
    """
    
    apis: list[tuple[str, list[str], callable, dict]]
    
    dbmgr: db.DatabaseInterface
    
    def api(self, path: str, methods: list[str], **kwargs):
        """Register an API.
        
        Args:
            path: path of this API, relative to this group.
            methods: HTTP methods.
            **kwargs: other arguments.
        """

        def decorator(handler):
            """Decorator."""
            self.apis.append((self.group_name+path, methods, handler, kwargs))
            return handler

        return decorator

    def __init__(self, dbmgr: db.DatabaseInterface):
        self.apis = []
        self.dbmgr = dbmgr
        
    def get_apis(self):
        """Get all APIs in this group.
        
        Returns:
            list of APIs.
        """
        return self.apis