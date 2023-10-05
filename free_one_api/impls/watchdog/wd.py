import asyncio

from ...models.watchdog import wd
from ...models.watchdog import task


class WatchDog(wd.AbsWatchDog):
    """WatchDog implementation."""
    
    def __init__(self):
        self.tasks = []
        
    async def run(self):
        cor = []
        
        for task in self.tasks:
            cor.append(task.loop())
            
        await asyncio.gather(*cor)
        
    def add_task(self, task: task.AbsTask):
        """Add a task."""
        self.tasks.append(task)
