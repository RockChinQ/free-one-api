import typing
import traceback
import uuid
import random

import bardapi as bard

from free_one_api.entities import request, response

from ...models import adapter
from ...models.adapter import llm
from ...entities import request, response, exceptions


@adapter.llm_adapter
class BardAdapter(llm.LLMLibAdapter):
    
    @classmethod
    def name(cls) -> str:
        return "dsdanielpark/Bard-API"
    
    @classmethod
    def description(self) -> str:
        return "Use dsdanielpark/Bard-API to access Claude web edition."

    def supported_models(self) -> list[str]:
        return [
            "gpt-3.5-turbo",
            "gpt-4"
        ]

    def function_call_supported(self) -> bool:
        return False

    def stream_mode_supported(self) -> bool:
        return False

    def multi_round_supported(self) -> bool:
        return True
    
    @classmethod
    def config_comment(cls) -> str:
        return \
"""Currently supports non stream mode only.
You should provide __Secure-1PSID as token extracted from cookies of Bard site.

{
    "token": "bQhxxxxxxxxxxx"
}

Method of getting __Secure-1PSID string, please refer to https://github.com/dsdanielpark/Bard-API
"""

    @classmethod
    def supported_path(cls) -> str:
        return "/v1/chat/completions"
    
    chatbot: bard.Bard
    
    def __init__(self, config: dict):
        self.config = config
        self.chatbot = bard.Bard(token=config['token'])
        
    async def test(self) -> (bool, str):
        try:
            self.chatbot.get_answer("hello, please reply 'hi' only.")
            return True, ""
        except Exception as e:
            traceback.print_exc()
            return False, str(e)

    async def query(self, req: request.Request) -> typing.AsyncGenerator[response.Response, None]:
        prompt = ""
        
        for msg in req.messages:
            prompt += f"{msg['role']}: {msg['content']}\n"
        
        prompt += "assistant: "
        
        random_int = random.randint(0, 1000000000)
        
        resp_text = self.chatbot.get_answer(prompt)['content']
        
        yield response.Response(
            id=random_int,
            finish_reason=response.FinishReason.STOP,
            normal_message=resp_text,
            function_call=None
        )
