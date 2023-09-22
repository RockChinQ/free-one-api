
class Request:
    """Request from http interface.

    Send to LLM lib adapters.
    """
    model: str
    """LLM model name."""

    messages: list[dict[str, str]]
    """OpenAI GPT style message list."""

    functions: list[dict[str, str]]
    """OpenAI ChatCompletion format function list."""

    stream: bool
    """True if this is a streaming request, processed by http interface level."""

    def __init__(
        self,
        model: str,
        messages: list[dict[str, str]],
        functions: list[dict[str, str]],
        stream: bool=False,
    ):
        self.model = model
        self.messages = messages
        self.functions = functions
        self.stream = stream
