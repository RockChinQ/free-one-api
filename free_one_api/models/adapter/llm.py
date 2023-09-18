import abc
import typing

from ...entities import response
from ...entities import request


class LLMLibAdapter(metaclass=abc.ABCMeta):
    """Base class for reverse engineering LLM Lib adapters."""
    
    config: dict

    @property
    def name(self) -> str:
        """Name of this adapter.
        
        Prefer the github path of LLM lib.
        e.g. acheong08/ChatGPT
        """
        return "not implemented"

    @property
    def description(self) -> str:
        """Description of this adapter."""
        return self.__class__.__doc__

    @property
    def supported_models(self) -> list[str]:
        """Return models supported by this reverse lib."""
        return []

    @property
    def function_call_supported(self) -> bool:
        """True if this adapter supports function call."""
        return False
    
    @property
    def multi_round_supported(self) -> bool:
        """True if this adapter supports multi round conversation."""
        return False

    @property
    def config_comment(self) -> str:
        """Comments of the config schema.
        
        Returns:
            str: comments, empty if no config needed.
        """
        return ""
    
    @property
    def supported_path(self) -> str:
        """Returns which path of OpenAI API should this adapter serve.
        
        e.g. /v1/chat/completion
        """
        return "/v1/chat/completion"

    @abc.abstractmethod
    def __init__(self, config: dict):
        """Init adapter with config.
        
        Args:
            config: config of this adapter.
        """
        self.config = config

    @abc.abstractmethod
    def get_config(self) -> dict:
        """Get set config.
        
        Returns:
            str: set config, empty if no config needed.
        """
        return {}

    @abc.abstractmethod
    async def test(self) -> (bool, str):
        """Test the adapter.
        
        Returns:
            bool: True if adapter can be used.
            str: error message, empty if no error
        """
        return False, "not implemented"

    @abc.abstractmethod
    async def query(self, req: request.Request) -> typing.Generator[response.Response, None, None]:
        """Query reply from LLM lib.
        
        Always in streaming mode. If upstream lib doesn't support streaming, just yield one time.
        """
        yield None
