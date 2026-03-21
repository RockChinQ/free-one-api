import asyncio
import typing
import traceback
import uuid
import random

import revkimi.kimichat as kimi

from free_one_api.entities import request, response

from ...models import adapter
from ...models.adapter import llm
from ...entities import request, response, exceptions
from ...models.channel import evaluation


@adapter.llm_adapter
class KimiAdapter(llm.LLMLibAdapter):

    @classmethod
    def name(cls) -> str:
        return "DrTang/revKimi"

    @classmethod
    def description(self) -> str:
        return "suck my pussy"

    def supported_models(self) -> list[str]:
        return [
            "gpt-3.5-turbo",
            "gpt-4",
            "gpt-4-1106-preview",
            "gpt-4-vision-preview",
            "gpt-4",
            "gpt-4-0314",
            "gpt-4-0613",
            "gpt-4-32k",
            "gpt-4-32k-0314",
            "gpt-4-32k-0613",
            "gpt-3.5-turbo-1106",
            "gpt-3.5-turbo",
            "gpt-3.5-turbo-16k",
            "gpt-3.5-turbo-0301",
            "gpt-3.5-turbo-0613",
            "gpt-3.5-turbo-16k-0613",
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
            """
            You should provide cookie string as `cookie` in config:
            {
                "cookie": "your cookie string"
            }

            """

    @classmethod
    def supported_path(cls) -> str:
        return "/v1/chat/completions"

    chatbot: kimi.Chatbot

    def __init__(self, config: dict, eval: evaluation.AbsChannelEvaluation):
        self.config = config
        self.eval = eval
        self.chatbot = kimi.Chatbot(
            cookies_str=config['cookie']
        )

    async def test(self) -> typing.Union[bool, str]:
        try:
            resp =self.chatbot.ask(
                prompt="Hello, reply 'hi' only.",
                conversation_id="",  # 会话ID（不填则会新建）
                timeout=10,  # 超时时间（默认10秒
                use_search=False  # 是否使用搜索
            )

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

        resp =self.chatbot.ask(
            prompt=prompt,
            conversation_id="",  # 会话ID（不填则会新建）
            timeout=10,  # 超时时间（默认10秒
            use_search=True  # 是否使用搜索
        )

        yield response.Response(
            id=random_int,
            finish_reason=response.FinishReason.NULL,
            normal_message=resp['text'],
            function_call=None
        )
