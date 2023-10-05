import abc

from . import task

class AbsWatchDog(metaclass=abc.ABCMeta):
    """Model of WatchDog."""
    
    tasks: list[task.AbsTask]
    """Added tasks."""
    
    @abc.abstractmethod
    async def run(self):
        """Run WatchDog system."""
        raise NotImplementedError
