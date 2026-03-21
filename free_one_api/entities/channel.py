import asyncio
import json
import time

import tiktoken

from ..models.adapter import llm
from ..models import adapter
from ..models.channel import evaluation
from ..impls.channel import eval as evl


class Channel:
    """Entity for channel."""
    id: int

    name: str
    """Name of this channel."""

    adapter: llm.LLMLibAdapter

    model_mapping: dict
    """Mapping model name to another model name."""

    enabled: bool

    latency: int
    
    eval: evaluation.AbsChannelEvaluation
    
    fail_count: int
    """Amount of sequential failures. Only in memory."""

    def __init__(self, id: int, name: str, adapter: llm.LLMLibAdapter, model_mapping: dict, enabled: bool, latency: int, eval: evaluation.AbsChannelEvaluation):
        self.id = id
        self.name = name
        self.adapter = adapter
        self.model_mapping = model_mapping
        self.enabled = enabled
        self.latency = latency
        self.eval = eval
        
        self.fail_count = 0

    @classmethod
    def dump_channel(cls, chan: 'Channel') -> dict:
        return {
            "id": chan.id,
            "name": chan.name,
            "adapter": adapter.dump_adapter(chan.adapter),
            "model_mapping": chan.model_mapping,
            "enabled": chan.enabled,
            "latency": chan.latency,
        }

    @classmethod
    def load_channel(cls, data: dict) -> 'Channel':
        
        eval = evl.ChannelEvaluation()
        return cls(
            data["id"],
            data["name"],
            adapter.load_adapter(data["adapter"], eval),
            data["model_mapping"],
            data["enabled"],
            data["latency"],
            eval,
        )

    def count_tokens(
        self,
        model: str,
        messages: list[str],
    ) -> int:
        """Count message tokens."""
        try:
            encoding = tiktoken.encoding_for_model(model)
        except KeyError:
            encoding = tiktoken.get_encoding("cl100k_base")
        num_tokens = 0
        for message in messages:
            for key, value in message.items():
                num_tokens += len(encoding.encode(str(value)))
        num_tokens += 3  # every reply is primed with <|start|>assistant<|message|>
        return num_tokens
    
    async def heartbeat(self, timeout: int=300) -> int:
        """Call adapter test, returns fail count."""
        
        try:
            start = time.time()
            succ, err = await asyncio.wait_for(self.adapter.test(), timeout=timeout)
            if succ:
                latency = int((time.time() - start)*100)/100
                self.fail_count = 0
                self.latency = latency
                return 0
            else:
                self.fail_count += 1
                return self.fail_count
        finally:
            self.fail_count += 1
            return self.fail_count

    def preserve_runtime_vars(
        self,
        chan1: 'Channel',
    ):
        """Preserve runtime variables from another channel.
        
        Args:
            chan1: channel to preserve from.
            
        """
        self.fail_count = chan1.fail_count
        self.eval = chan1.eval

    def __repr__(self) -> str:
        return f"<Channel {self.id} {self.name}>"
