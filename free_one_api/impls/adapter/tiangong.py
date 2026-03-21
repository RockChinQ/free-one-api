import asyncio
import typing
import traceback
import uuid
import random

import revTianGong.tiangong as tiangong

from free_one_api.entities import request, response

from ...models import adapter
from ...models.adapter import llm
from ...entities import request, response, exceptions
from ...models.channel import evaluation


@adapter.llm_adapter
class TianGongAdapter(llm.LLMLibAdapter):

    @classmethod
    def name(cls) -> str:
        return "DrTang/revTiangong"

    @classmethod
    def description(self) -> str:
        return "suck my dick"

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

    chatbot: tiangong.Chatbot

    def __init__(self, config: dict, eval: evaluation.AbsChannelEvaluation):
        self.config = config
        self.eval = eval
        self.chatbot = tiangong.Chatbot(
            cookies_str=config['cookie']
        )

    async def test(self) -> typing.Union[bool, str]:
        try:
            resp =await self.chatbot.ask(
                prompt="Hello, reply 'hi' only."
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

        resp =await (self.chatbot.ask(
            prompt=prompt,
        ))

        yield response.Response(
            id=random_int,
            finish_reason=response.FinishReason.NULL,
            normal_message=resp['texts'],
            function_call=None
        )
