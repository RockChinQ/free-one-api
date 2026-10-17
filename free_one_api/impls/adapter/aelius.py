"""Aelius 网页对话(https://aeliusai.com/chat)适配器。

站点没有公开的聊天 API,这里复用网页自己使用的接口:

    POST /api/v1/guest/session          建立游客身份(Cookie),无需账号
    POST /api/v1/chats                   新建网页对话
    POST /api/v1/chats/{id}/stream       发送一轮消息,响应为 SSE
    POST /api/v1/chats/{id}/regenerate   重答最后一条消息(上一轮失败后续跑)

多轮复用:网页对话的历史存在服务端,客户端每次带来的 messages 只是回放。只要
历史里上一条 assistant 内容与我们上一轮的答复一致、首条 user 消息一致,就沿用
同一个网页对话,只发送最后一条 user 消息;否则新建对话并一次性发送整理后的
完整历史。

只做正常网页 UI 的同款请求:不绕过验证码/Cloudflare、不伪造指纹。游客身份有
24 小时有效期,站点限流(约 12 次/分钟)时按 429 上报。
"""

import asyncio
import json
import random
import threading
import typing
from collections import OrderedDict

import requests

from ...models import adapter
from ...models.adapter import llm
from ...entities import request, response, exceptions
from ...models.channel import evaluation


DEFAULTS = {
    "base_url": "https://aeliusai.com",
    "timeout_secs": 300,
    "tools_enabled": False,
    "session_cookie": "",
    "max_cached_chats": 8,
    "user_agent": (
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
    ),
}


class _SessionExpired(Exception):
    """游客会话/Cookie 失效,需要重建身份。"""


class _ChatGone(Exception):
    """网页对话在服务端已不存在(游客会话过期或对话被删)。"""


class _ChatEntry:
    """一次网页对话的客户端侧索引,用于把下一轮请求接回同一个网页对话。"""

    __slots__ = ("chat_id", "first_user", "last_assistant", "pending_user")

    def __init__(self, chat_id: str, first_user: str):
        self.chat_id = chat_id
        self.first_user = first_user
        # 我们上一轮交给客户端的答复(用于比对回放历史)
        self.last_assistant = ""
        # 已发送但还没拿到答复的 user 消息;重试时走 /regenerate 而不是重发
        self.pending_user = ""


@adapter.llm_adapter
class AeliusAdapter(llm.LLMLibAdapter):
    """把 aeliusai.com/chat 的网页对话接成免费渠道,默认使用游客身份。"""

    # 类属性可被 config.yaml 的 adapters 段整体覆盖(setattr),也是配置缺省时的兜底。
    base_url: str = DEFAULTS["base_url"]
    timeout_secs: int = DEFAULTS["timeout_secs"]
    tools_enabled: bool = DEFAULTS["tools_enabled"]
    session_cookie: str = DEFAULTS["session_cookie"]
    max_cached_chats: int = DEFAULTS["max_cached_chats"]
    user_agent: str = DEFAULTS["user_agent"]

    @classmethod
    def name(cls) -> str:
        return "aeliusai/aelius-web"

    @classmethod
    def description(cls) -> str:
        return "Reverse-proxy the Aelius web chat (https://aeliusai.com/chat)."

    def supported_models(self) -> list[str]:
        return ["aelius", "aelius-web", "aelius-chat"]

    def function_call_supported(self) -> bool:
        return False

    def stream_mode_supported(self) -> bool:
        return True

    def multi_round_supported(self) -> bool:
        return True

    @classmethod
    def config_comment(cls) -> str:
        return (
            "直接反代 https://aeliusai.com/chat 的网页对话,默认自动建立游客身份,"
            "无需任何配置。可选配置项:\n"
            "session_cookie: 已登录账号的完整 Cookie 字符串(留空则用游客会话,游客上下文更小);\n"
            "tools_enabled: 是否让网页开启它的联网/工具开关(默认 false);\n"
            "timeout_secs: 单次回复超时,默认 300 秒;\n"
            "max_cached_chats: 内存里最多记住多少个网页对话用于多轮复用,默认 8;\n"
            "base_url / user_agent: 一般不需要改。\n"
            "网页会话与对话缓存只存在内存里,重启进程后重新建立。"
        )

    @classmethod
    def supported_path(cls) -> str:
        return "/v1/chat/completions"

    def __init__(self, config: dict, eval: evaluation.AbsChannelEvaluation):
        # 只保存 config 本身:每次请求现取现用,后台改完配置立即生效。
        self.config = config if isinstance(config, dict) else {}
        self.eval = eval

        self._http = requests.Session()
        self._session_ready = False
        self._session_lock = threading.Lock()
        self._chats: "OrderedDict[str, _ChatEntry]" = OrderedDict()
        self._lock = asyncio.Lock()

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

    def _timeout_secs(self) -> float:
        try:
            return max(30.0, float(self._setting("timeout_secs")))
        except (TypeError, ValueError):
            return float(DEFAULTS["timeout_secs"])

    def _tools_enabled(self) -> bool:
        return bool(self._setting("tools_enabled"))

    def _session_cookie(self) -> str:
        return str(self._setting("session_cookie") or "").strip()

    def _max_cached_chats(self) -> int:
        try:
            return max(1, int(self._setting("max_cached_chats")))
        except (TypeError, ValueError):
            return int(DEFAULTS["max_cached_chats"])

    def _user_agent(self) -> str:
        return str(self._setting("user_agent") or DEFAULTS["user_agent"])

    def get_config(self) -> dict:
        """回显生效配置(管理页读取渠道详情时用),不泄漏额外内容。"""
        return dict(self.config)

    # ------------------------------------------------------------------ HTTP 基础

    def _headers(self, sse: bool = False) -> dict:
        return {
            "User-Agent": self._user_agent(),
            "Accept": "text/event-stream" if sse else "application/json",
            "Accept-Language": "en-US,en;q=0.9",
            "Origin": self._base_url(),
            "Referer": self._base_url() + "/chat",
        }

    def _reset_session(self) -> None:
        try:
            self._http.close()
        except Exception:
            pass
        self._http = requests.Session()

    def _ensure_session(self) -> None:
        """游客身份懒加载:未配置 Cookie 时向站点申请一个游客会话。

        已配置 session_cookie 时直接把整段 Cookie 作为请求头发送,不再创建游客会话。
        """
        with self._session_lock:
            if self._session_ready:
                return

            cookie = self._session_cookie()
            if cookie:
                self._http.headers["Cookie"] = cookie
                self._session_ready = True
                return

            self._http.headers.pop("Cookie", None)
            try:
                resp = self._http.post(
                    self._base_url() + "/api/v1/guest/session",
                    json={},
                    headers=self._headers(),
                    timeout=20,
                )
            except requests.RequestException as e:
                raise exceptions.QueryHandlingError(
                    502,
                    "aelius_unreachable",
                    "无法连接 Aelius(" + self._base_url() + "): " + str(e),
                    type="api_error",
                ) from e

            if resp.status_code == 429:
                raise exceptions.QueryHandlingError(
                    429,
                    "aelius_rate_limited",
                    "Aelius 游客会话限流,请稍后再试。",
                    type="rate_limit_error",
                )
            if resp.status_code != 200:
                raise exceptions.QueryHandlingError(
                    502,
                    "aelius_guest_session_failed",
                    "Aelius 游客会话创建失败: HTTP " + str(resp.status_code),
                    type="api_error",
                )
            self._session_ready = True

    def _refresh_session(self) -> bool:
        """身份失效后重建;配置了固定 Cookie 时无法自动重建。"""
        if self._session_cookie():
            return False
        self._session_ready = False
        self._reset_session()
        self._ensure_session()
        return True

    @staticmethod
    def _error_from(resp: requests.Response, what: str) -> exceptions.QueryHandlingError:
        detail = ""
        try:
            detail = resp.text[:300]
        except Exception:
            pass
        if resp.status_code == 429:
            return exceptions.QueryHandlingError(
                429,
                "aelius_rate_limited",
                "Aelius 限流(" + what + "),请稍后再试。",
                type="rate_limit_error",
            )
        return exceptions.QueryHandlingError(
            502,
            "aelius_upstream_error",
            "Aelius " + what + " 返回 " + str(resp.status_code) + ": " + detail,
            type="api_error",
        )

    # ------------------------------------------------------------------ 对话接口

    def _create_chat(self) -> str:
        refreshed = False
        while True:
            self._ensure_session()
            try:
                resp = self._http.post(
                    self._base_url() + "/api/v1/chats",
                    json={"title": "New Chat"},
                    headers=self._headers(),
                    timeout=30,
                )
            except requests.RequestException as e:
                raise exceptions.QueryHandlingError(
                    502,
                    "aelius_unreachable",
                    "创建 Aelius 对话失败: " + str(e),
                    type="api_error",
                ) from e

            if resp.status_code == 401:
                if not refreshed and self._refresh_session():
                    refreshed = True
                    continue
                raise _SessionExpired()
            if resp.status_code >= 400:
                raise self._error_from(resp, "新建对话")

            try:
                return str(resp.json()["id"])
            except (ValueError, KeyError, TypeError) as e:
                raise exceptions.QueryHandlingError(
                    502,
                    "aelius_bad_response",
                    "Aelius 新建对话返回了无法解析的响应。",
                    type="api_error",
                ) from e

    def _post_stream(self, url: str, payload: dict) -> requests.Response:
        """打开 SSE 连接。身份失效时重建一次;对话不存在时抛 _ChatGone。"""
        refreshed = False
        while True:
            self._ensure_session()
            try:
                resp = self._http.post(
                    url,
                    json=payload,
                    headers=self._headers(sse=True),
                    stream=True,
                    timeout=(20, self._timeout_secs()),
                )
            except requests.RequestException as e:
                raise exceptions.QueryHandlingError(
                    502, "aelius_unreachable", "请求 Aelius 对话失败: " + str(e), type="api_error"
                ) from e

            if resp.status_code == 401:
                resp.close()
                if not refreshed and self._refresh_session():
                    refreshed = True
                    continue
                raise _SessionExpired()
            if resp.status_code in (404, 410):
                resp.close()
                raise _ChatGone()
            if resp.status_code >= 400:
                err = self._error_from(resp, "对话")
                resp.close()
                raise err
            return resp

    @staticmethod
    def _decode_frame(line: typing.Any) -> typing.Optional[dict]:
        if isinstance(line, bytes):
            line = line.decode("utf-8", "replace")
        line = (line or "").strip()
        if not line.startswith("data:"):
            return None
        raw = line[5:].strip()
        if not raw:
            return None
        try:
            data = json.loads(raw)
        except (TypeError, ValueError):
            return None
        return data if isinstance(data, dict) else None

    async def _stream_content(
        self,
        url: str,
        payload: dict,
        on_open: typing.Optional[typing.Callable[[], None]] = None,
    ) -> typing.AsyncGenerator[str, None]:
        """把 SSE 里的 content 增量逐段交给调用方;终止帧返回。

        on_open 在服务端确认收到这一轮之后触发(此时 user 消息已提交),
        用于区分"请求根本没进去"和"进去了但没答完"。
        """
        resp = await asyncio.to_thread(self._post_stream, url, payload)
        if on_open is not None:
            on_open()
        terminal = False
        try:
            lines = resp.iter_lines(decode_unicode=True)
            while True:
                line = await asyncio.to_thread(next, lines, None)
                if line is None:
                    break
                data = self._decode_frame(line)
                if not data:
                    continue

                content = data.get("content")
                if isinstance(content, str) and content:
                    yield content
                    continue

                if data.get("done") or data.get("stopped"):
                    terminal = True
                    break

                if data.get("error"):
                    raise exceptions.QueryHandlingError(
                        502,
                        "aelius_upstream_error",
                        "Aelius 返回错误: " + str(data.get("error")),
                        type="api_error",
                    )
                if data.get("context_full"):
                    raise exceptions.QueryHandlingError(
                        400,
                        "aelius_context_full",
                        "Aelius 网页对话上下文已满,请新开一段对话。",
                        type="invalid_request_error",
                    )
                if data.get("pool_full"):
                    raise exceptions.QueryHandlingError(
                        429,
                        "aelius_pool_full",
                        "Aelius 当前没有可用的上下文槽位,请稍后再试。",
                        type="overloaded_error",
                    )
                # think / tool / waiting / queued / model_tier / trimmed 等帧只影响网页 UI。
        except requests.RequestException as e:
            raise exceptions.QueryHandlingError(
                502, "aelius_stream_broken", "Aelius 流式响应中断: " + str(e), type="api_error"
            ) from e
        finally:
            try:
                await asyncio.to_thread(resp.close)
            except Exception:
                pass

        if not terminal:
            # 连接正常结束但没有 done/stopped 帧:网页端会当作掉线处理。
            raise exceptions.QueryHandlingError(
                502,
                "aelius_stream_incomplete",
                "Aelius 流式响应未正常结束。",
                type="api_error",
            )

    # ------------------------------------------------------------------ 多轮索引

    @staticmethod
    def _content_text(content: typing.Any) -> str:
        if content is None:
            return ""
        if isinstance(content, str):
            return content
        if isinstance(content, list):
            parts = []
            for part in content:
                if isinstance(part, str):
                    parts.append(part)
                elif isinstance(part, dict):
                    kind = part.get("type")
                    if kind in ("text", "input_text", "output_text"):
                        parts.append(str(part.get("text") or ""))
                    else:
                        # 图片/文件/音频都不转发:静默丢掉附件比报错更糟。
                        raise exceptions.QueryHandlingError(
                            400,
                            "aelius_attachment_not_supported",
                            "Aelius 适配器目前只转发文本消息,不支持图片/文件附件。",
                            type="invalid_request_error",
                        )
            return "".join(parts)
        return str(content)

    def _turns(self, messages: list) -> list[tuple[str, str]]:
        turns: list[tuple[str, str]] = []
        for msg in messages or []:
            if not isinstance(msg, dict):
                continue
            role = str(msg.get("role") or "user")
            text = self._content_text(msg.get("content"))
            if text.strip():
                turns.append((role, text))
        return turns

    @staticmethod
    def _first_user(turns: list[tuple[str, str]]) -> str:
        for role, text in turns:
            if role == "user":
                return text.strip()
        return ""

    @staticmethod
    def _previous_assistant(turns: list[tuple[str, str]]) -> str:
        for role, text in reversed(turns[:-1]):
            if role == "assistant":
                return text.strip()
        return ""

    def _render_transcript(self, turns: list[tuple[str, str]]) -> str:
        """新建网页对话时,把客户端回放的历史压成一条消息发过去。"""
        labels = {
            "system": "System",
            "developer": "System",
            "user": "User",
            "assistant": "Assistant",
            "tool": "Tool result",
            "function": "Tool result",
        }
        if len(turns) == 1 and turns[0][0] == "user":
            return turns[0][1]
        return "\n\n".join(
            labels.get(role, role.capitalize()) + ": " + text for role, text in turns
        )

    def _match(self, turns: list[tuple[str, str]]) -> typing.Optional[_ChatEntry]:
        """找到可以承接这一轮请求的网页对话。

        判据与网页端的多轮语义一致:首条 user 消息一致,且客户端历史里回放着
        我们上一轮的答复。上一轮已发出但没拿到答复时,按 pending_user 匹配,
        以便走 /regenerate 而不是把同一条消息再发一遍。
        """
        if len(turns) < 2 or turns[-1][0] != "user":
            return None
        first_user = self._first_user(turns)
        previous_assistant = self._previous_assistant(turns)
        last_user = turns[-1][1].strip()

        for key, entry in reversed(self._chats.items()):
            if entry.first_user != first_user:
                continue
            if entry.last_assistant and entry.last_assistant == previous_assistant:
                self._chats.move_to_end(key)
                return entry
            if entry.pending_user and entry.pending_user == last_user and not entry.last_assistant:
                self._chats.move_to_end(key)
                return entry
        return None

    def _remember(self, entry: _ChatEntry) -> None:
        key = entry.chat_id
        self._chats[key] = entry
        self._chats.move_to_end(key)
        while len(self._chats) > self._max_cached_chats():
            self._chats.popitem(last=False)

    def _mark_pending(self, entry: _ChatEntry, user_text: str) -> None:
        """服务端已收下这一轮,但还没答完:保留 user 文本,重试时走 /regenerate。"""
        entry.pending_user = user_text.strip()
        self._remember(entry)

    def _forget(self, entry: _ChatEntry) -> None:
        self._chats.pop(entry.chat_id, None)

    # ------------------------------------------------------------------ 探活

    def _probe(self) -> tuple[bool, str]:
        try:
            self._ensure_session()
            resp = self._http.get(
                self._base_url() + "/api/v1/user",
                headers=self._headers(),
                timeout=20,
            )
            if resp.status_code == 401 and not self._session_cookie():
                self._refresh_session()
                resp = self._http.get(
                    self._base_url() + "/api/v1/user",
                    headers=self._headers(),
                    timeout=20,
                )
            if resp.status_code == 200:
                return True, ""
            if resp.status_code == 401:
                return False, "Aelius Cookie 已失效,请更新 session_cookie。"
            return False, "Aelius /api/v1/user 返回 " + str(resp.status_code)
        except exceptions.QueryHandlingError as e:
            return False, e.message
        except Exception as e:
            return False, str(e)

    async def test(self) -> typing.Union[bool, str]:
        return await asyncio.to_thread(self._probe)

    # ------------------------------------------------------------------ 主流程

    async def query(self, req: request.Request) -> typing.AsyncGenerator[response.Response, None]:
        turns = self._turns(req.messages)
        if not turns:
            raise exceptions.QueryHandlingError(
                400,
                "aelius_empty_messages",
                "请求中没有可发送的消息。",
                type="invalid_request_error",
            )
        if turns[-1][0] != "user":
            raise exceptions.QueryHandlingError(
                400,
                "aelius_unexpected_last_role",
                "Aelius 适配器要求最后一条消息是 user。",
                type="invalid_request_error",
            )

        # 站点是低频个人用途的网页对话,串行化可以避免同一对话被并发写入。
        async with self._lock:
            resp_id = random.randint(0, 1000000000)
            entry = self._match(turns)
            retried_gone = False

            while True:
                try:
                    if entry is None:
                        chat_id = await asyncio.to_thread(self._create_chat)
                        entry = _ChatEntry(chat_id, self._first_user(turns))
                        self._remember(entry)
                        url = self._base_url() + "/api/v1/chats/" + chat_id + "/stream"
                        payload = {
                            "user_input": self._render_transcript(turns),
                            "tools_enabled": self._tools_enabled(),
                        }
                    elif entry.pending_user == turns[-1][1].strip():
                        # 上一轮消息已被网页端保存但没答完:续跑最后一条,不重复发送。
                        url = self._base_url() + "/api/v1/chats/" + entry.chat_id + "/regenerate"
                        payload = {"tools_enabled": self._tools_enabled()}
                    else:
                        url = self._base_url() + "/api/v1/chats/" + entry.chat_id + "/stream"
                        payload = {
                            "user_input": turns[-1][1],
                            "tools_enabled": self._tools_enabled(),
                        }

                    # 连接建立后服务端才真正收下 user 消息;在这之前失败不能标记 pending,
                    # 否则下一轮会对着一个没提交过的对话调用 /regenerate。
                    on_open = lambda e=entry, u=turns[-1][1]: self._mark_pending(e, u)
                    content: list[str] = []
                    try:
                        async for delta in self._stream_content(url, payload, on_open):
                            content.append(delta)
                            yield response.Response(
                                id=resp_id,
                                finish_reason=response.FinishReason.NULL,
                                normal_message=delta,
                                function_call=None,
                            )
                    except exceptions.QueryHandlingError as e:
                        # 这两类拒绝发生在生成之前,user 消息并没有保存。
                        if e.code in ("aelius_context_full", "aelius_pool_full"):
                            entry.pending_user = ""
                        raise

                    reply = "".join(content).strip()
                    if not reply:
                        raise exceptions.QueryHandlingError(
                            502,
                            "aelius_empty_reply",
                            "Aelius 返回了空答复。",
                            type="api_error",
                        )

                    entry.last_assistant = reply
                    entry.pending_user = ""
                    self._remember(entry)
                    yield response.Response(
                        id=resp_id,
                        finish_reason=response.FinishReason.STOP,
                        normal_message="",
                        function_call=None,
                    )
                    return

                except _ChatGone:
                    # 游客会话过期/对话被删:丢掉索引,按完整历史新开一个对话。
                    if entry is not None:
                        self._forget(entry)
                        entry = None
                    if retried_gone:
                        raise exceptions.QueryHandlingError(
                            502,
                            "aelius_chat_gone",
                            "Aelius 网页对话已失效,重建后仍不可用。",
                            type="api_error",
                        )
                    retried_gone = True
                    continue

                except _SessionExpired:
                    raise exceptions.QueryHandlingError(
                        401,
                        "aelius_auth_expired",
                        "Aelius 会话已失效,请在渠道配置里更新 session_cookie。",
                        type="authentication_error",
                    )
