
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

    tools: list[dict]

    tool_choice: object
    # OpenAI 风格的 tools 与 tool_choice,由适配器原样转交给上游。

    def __init__(
        self,
        model: str,
        messages: list[dict[str, str]],
        functions: list[dict[str, str]],
        stream: bool=False,
        tools: list[dict]=None,
        tool_choice: object=None,
    ):
        self.model = model
        self.messages = messages
        self.functions = functions
        self.stream = stream
        self.tools = tools
        self.tool_choice = tool_choice
