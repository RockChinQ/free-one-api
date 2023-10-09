from ...models.channel import evaluation


class ChannelEvaluation(evaluation.AbsChannelEvaluation):
    
    def __init__(self):
        self.records = []
    
    async def evaluate(self) -> float:
        """Implement evaluation algorithm here."""
        return 0.0
