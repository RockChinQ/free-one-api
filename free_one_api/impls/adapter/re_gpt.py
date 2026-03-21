import typing
import traceback
import uuid
import random

import requests
import re_gpt

from ...models import adapter
from ...models.adapter import llm
from ...entities import request
from ...entities import response, exceptions
from ...models.channel import evaluation


@adapter.llm_adapter
class ReGPTAdapter(llm.LLMLibAdapter):

    @classmethod
    def name(cls) -> str:
        return "Zai-Kun/reverse-engineered-chatgpt"

    @classmethod
    def description(self) -> str:
        return "Use Zai-Kun/reverse-engineered-chatgpt to access reverse engineering OpenAI ChatGPT web edition."

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
"""Please provide `session_token` to config as:

{
    "session_token": "your session"
}

Session token can be found from the cookies named `__Secure-next-auth.session-token` in the browser.
"""

    _chatbot: re_gpt.SyncChatGPT = None

    @property
    def chatbot(self) -> re_gpt.SyncChatGPT:
        if self._chatbot is None:
            self._chatbot = re_gpt.SyncChatGPT(**self.config)
        return self._chatbot

    @classmethod
    def supported_path(self) -> str:
        return "/v1/chat/completions"

    def __init__(self, config: dict, eval: evaluation.AbsChannelEvaluation):
        self.config = config
        self.eval = eval

    async def test(self) -> typing.Union[bool, str]:

        with self.chatbot as chatbot:
            conversation = chatbot.create_new_conversation()

            try:
                for message in conversation.chat("Hi, respond 'hello, world!' please."):
                    pass

                return True, ''
            except Exception as e:
                return False, str(e)
            finally:
                chatbot.delete_conversation(conversation.conversation_id)

    async def query(
        self,
        req: request.Request
    ) -> typing.AsyncGenerator[response.Response, None]:
        prompt = ""

        for msg in req.messages:
            prompt += f"{msg['role']}: {msg['content']}\n"

        prompt += "assistant: "

        random_int = random.randint(0, 1000000)

        with self.chatbot as chatbot:
            conversation = chatbot.create_new_conversation()
            try:

                for message in conversation.chat(
                    user_input=prompt
                ):
                    if message["content"] == "":
                        continue

                    yield response.Response(
                        id=random_int,
                        finish_reason=response.FinishReason.NULL,
                        normal_message=message["content"],
                        function_call=None
                    )
            except Exception as e:
                traceback.print_exc()
                raise e
            finally:
                chatbot.delete_conversation(conversation.conversation_id)

        yield response.Response(
            id=random_int,
            finish_reason=response.FinishReason.STOP,
            normal_message="",
            function_call=None
        )