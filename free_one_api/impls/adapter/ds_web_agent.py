import asyncio
import json
import random
import typing

import requests

from ...models import adapter
from ...models.adapter import llm
from ...entities import request, response, exceptions
from ...models.channel import evaluation


# 转发适配器:请求交给本机/同网络的 DS_WebAgent(Rust 版桥梁,驱动真实浏览器操作 chat.deepseek.com)。
# 部署方式见仓库根目录 docker-compose.dswebagent.yaml。
#
# 工具调用:请求里的 tools / tool_choice 原样转交上游;上游按 OpenAI 规范返回
# message.tool_calls 时,本适配器把它转成内部的 FunctionCall 并置 finish_reason=tool_calls,
# 由调用方执行工具、再把 role=tool + tool_call_id 的结果放回 messages 续跑。

DEFAULTS = {
    "base_url": "http://127.0.0.1:9178",
    "api_token": "local-dev-token",
    "default_model": "ds-web-search",
    "timeout_secs": 900,
}


def _url_to_path(url: str) -> str | None:
    """从 URL 或路径中提取本地文件路径。

    支持 file:// 前缀和直接路径。
    返回 None 表示不是本地路径（如 http/https/data URL）。
    """
    if not url:
        return None
    if url.startswith("file://"):
        return url[7:]
    if url.startswith(("http://", "https://", "data:", "ftp://")):
        return None
    return url


@adapter.llm_adapter
class DSWebAgentAdapter(llm.LLMLibAdapter):
    """把本地运行的 DS_WebAgent 接入 free-one-api。"""

    # 类属性可被 config.yaml 的 adapters 段整体覆盖(setattr),也是配置缺省时的兜底。
    base_url: str = DEFAULTS["base_url"]
    api_token: str = DEFAULTS["api_token"]
    default_model: str = DEFAULTS["default_model"]
    timeout_secs: int = DEFAULTS["timeout_secs"]

    @classmethod
    def name(cls) -> str:
        return "ds-webagent/deepseek-web"

    @classmethod
    def description(cls) -> str:
        return "Forward requests to a locally running DS_WebAgent (real browser on chat.deepseek.com)."

    def supported_models(self) -> list[str]:
        return ["ds-web-search", "ds-web-think", "ds-web-search-think"]

    def function_call_supported(self) -> bool:
        return True

    def stream_mode_supported(self) -> bool:
        return True

    def multi_round_supported(self) -> bool:
        return True

    @classmethod
    def config_comment(cls) -> str:
        return ('DS_WebAgent 是本机/同网络运行的桥梁服务,需先启动并在浏览器里手动登录 DeepSeek。'
                ' 配置项: base_url / api_token / default_model / timeout_secs;'
                ' api_token 必须与 DS_WebAgent 的 [server].api_token 一致。'
                ' 在管理页改完配置立刻生效,不需要重启。')

    @classmethod
    def supported_path(cls) -> str:
        return "/v1/chat/completions"

    def __init__(self, config: dict, eval: evaluation.AbsChannelEvaluation):
        # 只保存 config 本身:每次请求现取现用,后台改完配置立即生效。
        self.config = config if isinstance(config, dict) else {}
        self.eval = eval

    # ------------------------------------------------------------------ 配置读取

    def _setting(self, key: str):
        """渠道配置 -> 类属性(config.yaml 全局默认) -> 内置默认值。"""
        value = self.config.get(key) if isinstance(self.config, dict) else None
        if value is None or value == "":
            value = getattr(type(self), key, None)
        if value is None or value == "":
            value = DEFAULTS.get(key)
        return value

    def _base_url(self) -> str:
        return str(self._setting("base_url") or DEFAULTS["base_url"]).rstrip("/")

    def _api_token(self) -> str:
        return str(self._setting("api_token") or "")

    def _default_model(self) -> str:
        return str(self._setting("default_model") or DEFAULTS["default_model"])

    def _timeout_secs(self) -> int:
        try:
            return int(self._setting("timeout_secs"))
        except (TypeError, ValueError):
            return DEFAULTS["timeout_secs"]

    def get_config(self) -> dict:
        """回显生效配置(管理页读取渠道详情时用),不泄漏额外内容。"""
        return dict(self.config)

    # ------------------------------------------------------------------ 请求

    def _headers(self) -> dict:
        headers = {"Content-Type": "application/json"}
        token = self._api_token()
        if token:
            headers["Authorization"] = "Bearer " + token
        return headers

    def _resolve_model(self, requested: str) -> str:
        if requested in self.supported_models():
            return requested
        return self._default_model()

    def _build_payload(self, req: request.Request) -> dict:
        payload = {
            "model": self._resolve_model(req.model),
            "messages": req.messages,
            "stream": False,
        }
        tools = getattr(req, "tools", None)
        tool_choice = getattr(req, "tool_choice", None)
        if tools:
            payload["tools"] = tools
        if tool_choice is not None:
            payload["tool_choice"] = tool_choice
        if not tools and getattr(req, "functions", None):
            payload["functions"] = req.functions
        return payload

    @staticmethod
    def _extract_file_paths(messages: list) -> list[str]:
        """从 OpenAI 格式消息中提取本地文件/图片路径。

        支持以下格式:
        - {"type": "image_url", "image_url": {"url": "/path/to/file"}}
        - {"type": "file_url", "file_url": {"url": "/path/to/file"}}
        - {"type": "file_path", "path": "/path/to/file"}
        - file:// 前缀的 URL
        """
        paths = []
        for msg in messages:
            content = msg.get("content")
            if not isinstance(content, list):
                continue
            for part in content:
                if not isinstance(part, dict):
                    continue
                part_type = part.get("type")
                if part_type == "image_url":
                    url = part.get("image_url", {}).get("url", "")
                    path = _url_to_path(url)
                    if path:
                        paths.append(path)
                elif part_type == "file_url":
                    url = part.get("file_url", {}).get("url", "")
                    path = _url_to_path(url)
                    if path:
                        paths.append(path)
                elif part_type == "file_path":
                    path = part.get("path", "")
                    if path:
                        paths.append(path)
        return paths

    def _post_chat(self, payload: dict) -> dict:
        resp = requests.post(
            self._base_url() + "/v1/chat/completions",
            json=payload,
            headers=self._headers(),
            timeout=self._timeout_secs(),
        )
        if resp.status_code != 200:
            raise exceptions.QueryHandlingError(
                502,
                "ds_webagent_upstream_error",
                "DS_WebAgent 返回 " + str(resp.status_code) + ": " + resp.text[:500],
                type="api_error",
            )
        return resp.json()

    @staticmethod
    def _extract_message(data: dict) -> dict:
        choices = data.get("choices") or []
        if not choices:
            raise exceptions.QueryHandlingError(
                502,
                "ds_webagent_empty_response",
                "DS_WebAgent 响应中没有 choices",
                type="api_error",
            )
        return choices[0].get("message") or {}

    @staticmethod
    def _parse_tool_call(message: dict):
        """上游按 OpenAI 规范返回 tool_calls;本项目的内部结构一次只承载一个工具调用。"""
        tool_calls = message.get("tool_calls") or []
        if not tool_calls:
            return None
        call = tool_calls[0] or {}
        function = call.get("function") or {}
        raw_arguments = function.get("arguments")
        if isinstance(raw_arguments, dict):
            arguments = raw_arguments
        else:
            try:
                arguments = json.loads(raw_arguments) if raw_arguments else {}
            except (TypeError, ValueError):
                arguments = {"_raw": raw_arguments}
            if not isinstance(arguments, dict):
                arguments = {"_raw": arguments}
        return response.FunctionCall(
            function.get("name") or "",
            arguments,
            call.get("id"),
        )

    # ------------------------------------------------------------------ 探活

    async def test(self) -> typing.Union[bool, str]:
        base_url = self._base_url()
        try:
            resp = await asyncio.to_thread(requests.get, base_url + "/health", timeout=15)
        except Exception as e:
            return False, "连不上 DS_WebAgent(" + base_url + "): " + str(e)
        try:
            data = resp.json()
        except Exception:
            return False, "DS_WebAgent /health 返回非 JSON(" + str(resp.status_code) + ")"
        if data.get("ready"):
            return True, ""
        return False, "DS_WebAgent 未就绪: " + str(data.get("detail") or data.get("status"))

    # ------------------------------------------------------------------ 主流程

    async def query(self, req: request.Request) -> typing.AsyncGenerator[response.Response, None]:
        payload = self._build_payload(req)
        resp_id = random.randint(0, 1000000000)

        try:
            data = await asyncio.to_thread(self._post_chat, payload)
        except exceptions.QueryHandlingError:
            raise
        except Exception as e:
            raise exceptions.QueryHandlingError(
                502,
                "ds_webagent_unreachable",
                "调用 DS_WebAgent 失败(" + self._base_url() + "): " + str(e),
                type="api_error",
            ) from e

        message = self._extract_message(data)
        tool_call = self._parse_tool_call(message)
        text = message.get("content") or ""

        if tool_call is not None:
            yield response.Response(
                id=resp_id,
                finish_reason=response.FinishReason.FUNCTION_CALL,
                normal_message=text or None,
                function_call=tool_call,
            )
            return

        yield response.Response(
            id=resp_id,
            finish_reason=response.FinishReason.STOP,
            normal_message=text,
            function_call=None,
        )
