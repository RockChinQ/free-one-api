import abc
import asyncio
import traceback
import logging


class AbsTask(metaclass=abc.ABCMeta):
    """Task of WatchDog.
    
    A task may need instances of other modules to work. These dependencies should be set by outer constructor.
    Task's delay and interval may be set by outer constructor also, and be scheduled by WatchDog implementation.
    """
    
    delay: int = 0
    """Delay before first trigger."""
    
    interval: int = 60
    """Interval between two triggers."""
    
    @abc.abstractmethod
    async def trigger(self):
        """Trigger this task."""
        raise NotImplementedError
    
    async def loop(self):
        """Loop this task."""
        await asyncio.sleep(self.delay)
        while True:
            try:
                await self.trigger()
            except Exception:
                logging.warn(f"Failed to trigger task {self.__class__.__name__}, traceback: {traceback.format_exc()}")
            await asyncio.sleep(self.interval)
