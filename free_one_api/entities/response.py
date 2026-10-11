"""Entity for both one-time and streaming response."""
import enum


class FinishReason(enum.Enum):
    """Finish reason type enum."""

    NULL = None
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

    # 上游(OpenAI 风格)返回的 tool_call id:原样交还客户端,
    # 客户端用它以 role=tool + tool_call_id 回传执行结果。
    id: str

    def __init__(self, function_name: str, arguments: dict, id: str = None):
        self.function_name = function_name
        self.arguments = arguments
        self.id = id


class Response:
    """Entity for both one-time and streaming response.
    
    Be created by LLM lib adapters. Pass between LLM lib adapters and protocol wrapper(http interface).
    """
    id: str
    """Set by upstream lib, used to identify this response."""

    finish_reason: FinishReason
    """The reason why this response responded."""

    normal_message: str
    """Text of the normal message."""

    function_call: FunctionCall
    """Function call."""

    def __init__(self, id: str, finish_reason: FinishReason, normal_message: str = None, function_call: FunctionCall = None):
        self.id = id
        self.finish_reason = finish_reason
        self.normal_message = normal_message
        self.function_call = function_call
    
    def __str__(self) -> str:
        return f"Response({self.id}, {self.finish_reason}, {self.normal_message}, {self.function_call})"    
    