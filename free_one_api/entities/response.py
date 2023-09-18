"""Entity for both one-time and streaming response."""
import enum


class FinishReason(enum.Enum):
    """Finish reason type enum."""

    NULL = "null"
    """Null if this is a part of a streaming response."""

    FUNCTION_CALL = "function_call"
    """Function call requested in this response, the ending of a streaming response."""

    STOP = "stop"
    """Normal message response, the ending of a streaming response."""

    LENGTH = "length"
    """Reponse exceeded the length limit, the ending of a streaming response."""


class FunctionCall:
    """Function call."""

    function_name: str

    arguments: dict

    def __init__(self, function_name: str, arguments: dict):
        self.function_name = function_name
        self.arguments = arguments


class Response:
    """Entity for both one-time and streaming response.
    
    Be created by LLM lib adapters. Pass between LLM lib adapters and protocol wrapper(http interface).
    """

    finish_reason: FinishReason
    """The reason why this response responded."""

    normal_message: str
    """Text of the normal message."""

    function_call: FunctionCall
    """Function call."""
