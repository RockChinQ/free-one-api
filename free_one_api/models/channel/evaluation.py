import abc
import asyncio
import time
import enum
import logging


class Record:
    
    start_time: float = 0.0
    """Start time of request."""
    
    end_time: float = -1.0
    """End time of request."""
    
    latency: float = -1.0
    """Latency of request."""
    
    req_messages_length: int = 0
    """Request messages."""
    
    resp_message_length: int = 0
    """Response message length."""
    
    stream: bool = False
    """Whether the request is stream mode."""
    
    success: bool = False
    """Whether the request is successful."""
    
    error: Exception = None
    """Error of request."""
    
    def __init__(
        self,
        start_time: float=0.0,
        end_time: float=-1.0,
        latency: float=-1.0,
        req_messages_length: int=0,
        resp_message_length: int=0,
        success: bool=False,
        error: Exception=None,
    ):
        self.start_time = start_time
        self.end_time = end_time
        self.latency = latency
        self.req_messages_length = req_messages_length
        self.resp_message_length = resp_message_length
        self.success = success
        self.error = error
        
    def commit(self):
        self.end_time = time.time()
        logging.debug(f"Commit record {self}")

    def __str__(self) -> str:
        return f"""Record(start_time={self.start_time}, 
end_time={self.end_time}, 
latency={self.latency}, 
req_messages_length={self.req_messages_length}, 
resp_message_length={self.resp_message_length}, 
stream={self.stream}, 
success={self.success}, 
error={self.error}
)""".replace("\n", "")


class AbsChannelEvaluation(metaclass=abc.ABCMeta):
    """Evaluation for channel.
    
    Takes performance or other index into account and give a score of channel.
    """
    records: list[Record]
    
    def add_record(self, record: Record):
        """Add a record.
        
        Args:
            record (Record): Record to add.
        """
        self.records.append(record)

    @abc.abstractmethod
    async def evaluate(self) -> float:
        """Evaluate the channel.
        
        Returns:
            float: Score of channel.
        """
        pass
