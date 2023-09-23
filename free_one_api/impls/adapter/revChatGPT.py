import typing
import traceback
import uuid
import random

import revChatGPT.V1 as chatgpt

from ...models import adapter
from ...models.adapter import llm
from ...entities import request
from ...entities import response, exceptions


@adapter.llm_adapter
class RevChatGPTAdapter(llm.LLMLibAdapter):
    
    @classmethod
    def name(cls) -> str:
        return "acheong08/ChatGPT"
    
    @classmethod
    def description(self) -> str:
        return "Use acheong08/ChatGPT to access reverse engineering OpenAI ChatGPT web edition."

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
        return "/v1/chat/completions"
    
    chatbot: chatgpt.AsyncChatbot
    
    def __init__(self, config: dict):
        self.config = config
        self.chatbot = chatgpt.AsyncChatbot(
            config=config,
            base_url="https://chatproxy.rockchin.top/api/"
        )
    
    async def test(self) -> (bool, str):
        try:
            prev_text = ""
            async for data in self.chatbot.ask(
                "Hi, respond 'Hello, world!' please.",
            ):
                message = data["message"][len(prev_text):]
                prev_text = data["message"]
            return True, ""
        except Exception as e:
            traceback.print_exc()
            return False, str(e)
    
    async def query(self, req: request.Request) -> typing.Generator[response.Response, None, None]:        
        new_messages = []
        for i in range(len(req.messages)):
            new_messages.append({
                "id": str(uuid.uuid4()),
                "author": {"role": req.messages[i]['role']},
                "content": {
                    "content_type": "text",
                    "parts": [
                        req.messages[i]['content']
                    ]
                }
            })
        
        random_int = random.randint(0, 1000000000)
        
        prev_text = ""
        
        try:
        
            async for data in self.chatbot.post_messages(
                messages=new_messages,
            ):
                message = data["message"][len(prev_text):]
                prev_text = data["message"]
                
                yield response.Response(
                    id=random_int,
                    finish_reason=response.FinishReason.NULL,
                    normal_message=message,
                    function_call=None
                )
                random_int += 1
                
            yield response.Response(
                id=random_int,
                finish_reason=response.FinishReason.STOP,
                normal_message="",
                function_call=None
            )
        except chatgpt.t.Error as e:
            assert isinstance(e, chatgpt.t.ErrorType)
            code = e.code.name.lower()
            
            raise exceptions.QueryHandlingError(
                status_code=500,
                code=code,
                message=e.message,
            )
