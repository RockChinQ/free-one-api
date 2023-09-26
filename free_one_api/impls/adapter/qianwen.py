import typing
import traceback
import uuid
import random

import revTongYi.qianwen as qwen

from free_one_api.entities import request, response

from ...models import adapter
from ...models.adapter import llm
from ...entities import request, response, exceptions


@adapter.llm_adapter
class QianWenAdapter(llm.LLMLibAdapter):
    
    @classmethod
    def name(cls) -> str:
        return "xw5xr6/revTongYi"
    
    @classmethod
    def description(self) -> str:
        return "Use xw5xr6/revTongYi to access Aliyun TongYi QianWen."

    def supported_models(self) -> list[str]:
        return [
            "gpt-3.5-turbo",
            "gpt-4"
        ]

    def function_call_supported(self) -> bool:
        return False

    def stream_mode_supported(self) -> bool:
        return True

    def multi_round_supported(self) -> bool:
        return True
    
    @classmethod
    def config_comment(cls) -> str:
        return \
"""RevTongYi use cookies that can be extracted from https://qianwen.aliyun.com/
You should provide cookie string as `cookie` in config:
{
    "cookie": "your cookie string"
}

Method of getting cookie string, please refer to https://github.com/xw5xr6/revTongYi
"""

    @classmethod
    def supported_path(cls) -> str:
        return "/v1/chat/completions"
    
    chatbot: qwen.Chatbot
    
    def __init__(self, config: dict):
        self.config = config
        self.chatbot = qwen.Chatbot(
            cookies_str=config['cookie']
        )
        
    async def test(self) -> (bool, str):
        try:
            self.chatbot.create_session("Hello, reply 'hi' only.")
            resp = self.chatbot.ask(
                "Hello, reply 'hi' only.",
            )
            
            self.chatbot.delete_session(self.chatbot.sessionId)
            
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
        
        prev_text = ""
        self.chatbot.create_session(prompt)
        
        for resp in self.chatbot.ask(
            prompt=prompt,
            stream=True,
        ):
            
            yield response.Response(
                id=random_int,
                finish_reason=response.FinishReason.NULL,
                normal_message=resp['content'][0].replace(prev_text, ""),
                function_call=None
            )
            prev_text = resp['content'][0]
        
        self.chatbot.delete_session(self.chatbot.sessionId)
        
        yield response.Response(
            id=random_int,
            finish_reason=response.FinishReason.STOP,
            normal_message="",
            function_call=None
        )
