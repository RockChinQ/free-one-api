import typing

from ...models import adapter
from ...models.adapter import llm
from ...entities import request
from ...entities import response


@adapter.llm_adapter
class RevChatGPTAdapter(llm.LLMLibAdapter):
    
    @classmethod
    def name(cls) -> str:
        return "acheong08/ChatGPT"
    
    @classmethod
    def description(self) -> str:
        return "Use acheong08/ChatGPT to access reverse engineering OpenAI ChatGPT web edition."

    @property
    def supported_models(self) -> list[str]:
        return [
            "gpt-3.5-turbo",
            "gpt-4"
        ]
        
    @property
    def function_call_supported(self) -> bool:
        return False
    
    @property
    def multi_round_supported(self) -> bool:
        return True
    
    @classmethod
    def config_comment(cls) -> str:
        return \
"""You can provide `access_token` or `email/password` to config as:
{
    "access_token": "your access token",
}
or
{
    "email": "your email",
    "password": "your password"
}
and you can also provide other optional params supported by acheong08/ChatGPT:
{
  "conversation_id": "UUID...",
  "parent_id": "UUID...",
  "proxy": "...",
  "model": "gpt-4", // gpt-4-browsing, text-davinci-002-render-sha, gpt-4, gpt-4-plugins
  "plugin_ids": ["plugin-d1d6eb04-3375-40aa-940a-c2fc57ce0f51"], // Wolfram Alpha example
  "disable_history": true,
  "PUID": "<_puid cookie for plus accounts>", // Only if you have a plus account and use GPT-4
  "unverified_plugin_domains":["showme.redstarplugin.com"] // Unverfied plugins to install
}

Please refer to https://github.com/acheong08/ChatGPT
"""
    
    @classmethod
    def supported_path(self) -> str:
        return "/v1/chat/completion"
    
    def __init__(self, config: dict):
        self.config = config
    
    
    async def test(self) -> (bool, str):
        return True, ""
    
    async def query(self, req: request.Request) -> typing.Generator[response.Response, None, None]:
        yield response.Response(
            finish_reason=response.FinishReason.STOP,
            normal_message="",
            function_call=None
        )