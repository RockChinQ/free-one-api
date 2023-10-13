import random
import time

from ...models.channel import evaluation


class ChannelEvaluation(evaluation.AbsChannelEvaluation):
    
    init_time: int
    
    def __init__(self):
        self.init_time = time.time()
        self.records = []
    
    async def evaluate(self) -> float:
        """Evaluate channel.
        
        Sum up:
        
         - `0 - last5RecordsAverageLatency`
         - `lastUseTime`, 0 if using
        """
        records_reverse = self.records[::-1]
        
        now_time = time.time()
        
        lastUseTime = -1
        
        last5Records: list[evaluation.Record] = []
        
        if len(records_reverse) == 0:
            lastUseTime = now_time - self.init_time
        
        for record in records_reverse:
            if lastUseTime == -1:
                if record.end_time < 0:  # querying
                    lastUseTime = 0
                else:  # last query committed
                    lastUseTime = now_time - record.end_time

            if len(last5Records) >= 5:
                break
            if record.end_time > 0:  # committed
                if record.success:
                    last5Records.append(record)

        last5RecordsAverageLatency = sum([record.latency for record in last5Records]) / len(last5Records) if len(last5Records) > 0 else 0
        
        return (0 - last5RecordsAverageLatency) + lastUseTime