import typing
import traceback
import uuid
import logging
import random

import g4f

g4f.version_check = False

from free_one_api.entities import request, response

from ...models import adapter
from ...models.adapter import llm
from ...entities import request, response, exceptions
from ...models.channel import evaluation


@adapter.llm_adapter
class GPT4FreeAdapter(llm.LLMLibAdapter):
    
    @classmethod
    def name(cls) -> str:
        return "xtekky/gpt4free"
    
    @classmethod
    def description(self) -> str:
        return "Use xtekky/gpt4free to access lots of GPT providers."

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
"""GPT4Free is so unstable that it is not recommended to use.
You don't need to provide any authentification.

Please refer to https://github.com/xtekky/gpt4free
"""

    @classmethod
    def supported_path(cls) -> str:
        return "/v1/chat/completions"
    
    def __init__(self, config: dict, eval: evaluation.AbsChannelEvaluation):
        self.config = config
        self.eval = eval
        
    _use_provider: g4f.Provider = None
    _use_stream_provider: g4f.Provider = None
    
    async def use_provider(self, stream: bool) -> g4f.Provider.BaseProvider:
        if self._use_provider is None:
            await self._select_provider()
        if stream and self._use_stream_provider is not None:
            return self._use_stream_provider
        return self._use_provider
    
    async def _select_provider(self):
        non_stream_tested = False
        if self._use_provider is not None:
            try:
                resp = await g4f.ChatCompletion.create_async(
                    model="gpt-3.5-turbo",
                    messages=[
                        {
                            "role": "user",
                            "content": "Hi, My name is Rock."
                        }
                    ],
                    provider=self._use_provider
                )
                non_stream_tested = True
            except Exception as e:
                self._use_provider = None
        if non_stream_tested and self._use_stream_provider is not None:
            try:
                resp = self._use_stream_provider.create_async_generator(
                    model="gpt-3.5-turbo",
                    messages=[
                        {
                            "role": "user",
                            "content": "Hi, My name is Rock."
                        }
                    ]
                )
                async for _ in resp:
                    return
            except Exception as e:
                self._use_stream_provider = None
        
        self._use_provider = None
        self._use_stream_provider = None

        from g4f.Provider import __all__ as providers

        exclude = [
            'Acytoo',
            'BaseProvider',
            'Bing'
        ]

        for provider in providers:

            # print("Testing provider", provider)
            # logging.info("Testing provider %s", provider)

            if provider in exclude:
                continue

            provider = getattr(g4f.Provider, provider)
            
            try:
                assert hasattr(provider, 'supports_stream')
                resp = await g4f.ChatCompletion.create_async(
                    model="gpt-3.5-turbo",
                    messages=[
                        {
                            "role": "user",
                            "content": "Hi, My name is Rock."
                        }
                    ],
                    provider=provider
                )

                if 'Rock' in resp and '<' not in resp:
                    if self._use_provider is None:
                        self._use_provider = provider
                    
                    if provider.supports_stream:
                        try:
                            assert hasattr(provider, 'create_async_generator')
                            resp = provider.create_async_generator(
                                model="gpt-3.5-turbo",
                                messages=[
                                    {
                                        "role": "user",
                                        "content": "Hi, My name is Rock."
                                    }
                                ],
                                timeout=120
                            )
                            async for _ in resp:
                                pass
                            if self._use_stream_provider is None:
                                self._use_stream_provider = provider
                        except Exception as e:
                            traceback.print_exc()
                            print("provider", provider, "does not really support stream mode")
                        
                    
                    if self._use_provider is not None and self._use_stream_provider is not None:
                        print("selected provider", self._use_provider, self._use_stream_provider)
                        break
            except Exception as e:
                # traceback.print_exc()
                continue
            
        if self._use_provider is None:
            raise exceptions.QueryHandlingError(404, "no_provider_found", "No provider available.")

    async def test(self) -> (bool, str):
        try:
            await self._select_provider()
            resp = await g4f.ChatCompletion.create_async(
                model="gpt-3.5-turbo",
                messages=[{
                    "role": "user",
                    "content": "Hello, please reply 'hi' only."
                }],
                provider=await self.use_provider(stream=False)
            )
            return True, ""
        except Exception as e:
            traceback.print_exc()
            return False, str(e)
        
    async def query(self, req: request.Request) -> typing.AsyncGenerator[response.Response, None]:
        provider = await self.use_provider(stream=True)
        
        if not req.stream:
            resp = await g4f.ChatCompletion.create_async(
                model=req.model,
                messages=req.messages,
                provider=provider,
                timeout=180
            )
        else:
            resp = provider.create_async_generator(
                model=req.model,
                messages=req.messages,
                timeout=180
            )
        
        if isinstance(resp, typing.Generator):
            for resp_text in resp:
                random_int = random.randint(0, 1000000000)
                yield response.Response(
                    id=random_int,
                    finish_reason=response.FinishReason.NULL,
                    normal_message=resp_text,
                    function_call=None
                )
            yield response.Response(
                id=random_int,
                finish_reason=response.FinishReason.STOP,
                normal_message="",
                function_call=None
            )
        elif isinstance(resp, typing.AsyncGenerator):
            async for resp_text in resp:
            
                random_int = random.randint(0, 1000000000)
                
                yield response.Response(
                    id=random_int,
                    finish_reason=response.FinishReason.NULL,
                    normal_message=resp_text,
                    function_call=None
                )
            yield response.Response(
                id=random_int,
                finish_reason=response.FinishReason.STOP,
                normal_message="",
                function_call=None
            )
        else:
            random_int = random.randint(0, 1000000000)
            
            yield response.Response(
                id=random_int,
                finish_reason=response.FinishReason.STOP,
                normal_message=resp,
                function_call=None
            )
