import abc
import time
import enum


class RecordType(enum.Enum):
    
    RESPONSE_DELAY = 1
    
    RESPONSE_TOTAL_TIME = 2
    
    RESPONSE_LENGTH = 3
    

class Record:
    
    type: RecordType
    value: float
    time: float
    
    def __init__(self, type: RecordType, value: float):
        self.type = type
        self.value = value
        self.time = time.time()


class AbsChannelEvaluation(metaclass=abc.ABCMeta):
    """Evaluation for channel.
    
    Takes performance or other index into account and give a score of channel.
    """
    TYPES = RecordType
    
    records: dict[RecordType, list[Record]]
    
    def record(self, type: RecordType, value: float):
        """Record a value.
        
        Args:
            type (RecordType): Type of value.
            value (float): Value to record.
        """
        if type not in self.records:
            self.records[type] = []
        self.records[type].append(Record(type, value))

    @abc.abstractmethod
    async def evaluate(self) -> float:
        """Evaluate the channel.
        
        Returns:
            float: Score of channel.
        """
        pass
