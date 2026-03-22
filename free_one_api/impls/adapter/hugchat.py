import typing
import traceback
import uuid
import random

import requests

import hugchat.hugchat as hugchat
import hugchat.login as login

from ...models import adapter
from ...models.adapter import llm
from ...entities import request
from ...entities import response, exceptions
from ...models.channel import evaluation


@adapter.llm_adapter
class HuggingChatAdapter(llm.LLMLibAdapter):

    @classmethod
    def name(cls) -> str:
        return "Soulter/hugging-chat-api"

    @classmethod
    def description(self) -> str:
        return "Use Soulter/hugging-chat-api to access reverse engineering huggingchat."

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
            """Please provide email and passwd to sign up for HuggingChat:
             
            {
                "email": "your email",
                "passwd": "your password"
            }
            
            Please refer to https://github.com/Soulter/hugging-chat-api
            """

    @classmethod
    def supported_path(self) -> str:
        return "/v1/chat/completions"

    _chatbot: hugchat.ChatBot = None

    @property
    def chatbot(self) -> hugchat.ChatBot:
        if self._chatbot is None:
            sign = login.Login(self.config['email'], self.config['passwd'])
            cookie: requests.sessions.RequestsCookieJar = None
            try:
                cookie = sign.loadCookiesFromDir("data/hugchatCookies")
            except:
                cookie = sign.login()
                sign.saveCookiesToDir("data/hugchatCookies")

            self._chatbot = hugchat.ChatBot(cookies=cookie.get_dict())
        return self._chatbot

    def __init__(self, config: dict, eval: evaluation.AbsChannelEvaluation):
        self.config = config
        self.eval = eval

    async def test(self) -> typing.Union[bool, str]:
        try:
            self.chatbot.change_conversation(self.chatbot.new_conversation())
            for data in self.chatbot.chat("Hi, respond 'Hello, world!' please."):
                pass

            self.chatbot.delete_conversation(self.chatbot.current_conversation)

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
        self.chatbot.change_conversation(self.chatbot.new_conversation())

        for resp in self.chatbot.chat(prompt):
            if resp is not None:
                yield response.Response(
                    id=random_int,
                    finish_reason=response.FinishReason.NULL,
                    normal_message=resp['token'],
                    function_call=None
                )
        self.chatbot.delete_conversation(self.chatbot.current_conversation)

        yield response.Response(
            id=random_int,
            finish_reason=response.FinishReason.STOP,
            normal_message="",
            function_call=None
        )
