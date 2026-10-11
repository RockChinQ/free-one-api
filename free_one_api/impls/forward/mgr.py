import time
import json
import string
import random
import logging
import typing
import traceback

import quart

from ...models.forward import mgr as forwardmgr
from ...models.channel import mgr as channelmgr
from ...models.key import mgr as apikeymgr
from ...entities import channel, apikey, request, response, exceptions
from ...common import randomad
from ...models.channel import evaluation
from . import protocol as protocolmod


# 渠道适配器一律按 OpenAI Chat Completions 协议与上游通信,
# 因此 Anthropic / Responses 路径在选渠道时统一归一化到这个路径。
ADAPTER_PATH = "/v1/chat/completions"


def finish_reason_value(resp: response.Response) -> str:
    # OpenAI 用 tool_calls 表示模型要调工具;本项目内部枚举叫 FUNCTION_CALL。
    if resp is None:
        return "stop"
    if resp.finish_reason == response.FinishReason.FUNCTION_CALL:
        return "tool_calls"
    return resp.finish_reason.value


def tool_call_json(fc: response.FunctionCall, fallback_id) -> dict:
    arguments = fc.arguments
    if isinstance(arguments, str):
        arguments_str = arguments
    else:
        arguments_str = json.dumps(arguments if arguments is not None else {}, ensure_ascii=False)
    return {
        "id": fc.id or ("call_" + str(fallback_id)),
        "type": "function",
        "function": {
            "name": fc.function_name,
            "arguments": arguments_str,
        },
    }


def delta_of(resp: response.Response, fallback_id) -> dict:
    delta = {}
    if resp.normal_message:
        delta["content"] = resp.normal_message
    if resp.function_call is not None:
        call = tool_call_json(resp.function_call, fallback_id)
        call["index"] = 0
        delta["tool_calls"] = [call]
    return delta


def build_message(normal_message: str, fc, fallback_id) -> dict:
    if fc is None:
        return {"role": "assistant", "content": normal_message}
    return {
        "role": "assistant",
        "content": normal_message or None,
        "tool_calls": [tool_call_json(fc, fallback_id)],
    }


class ForwardManager(forwardmgr.AbsForwardManager):

    def __init__(self, chanmgr: channelmgr.AbsChannelManager, keymgr: apikeymgr.AbsAPIKeyManager):
        self.chanmgr = chanmgr
        self.keymgr = keymgr

    async def _collect(
        self,
        chan: channel.Channel,
        req: request.Request,
        record: evaluation.Record,
        before: float,
    ):
        """把适配器的流式响应聚合成完整结果(text + function_call)。"""
        normal_message = ""
        function_call: response.FunctionCall = None
        resp_tmp: response.Response = None
        try:
            async for resp in chan.adapter.query(req):
                if record.latency < 0:
                    record.latency = time.time() - before
                logging.debug("resp: {}".format(resp))
                if resp.normal_message is not None:
                    resp_tmp = resp
                    normal_message += resp.normal_message
                    record.resp_message_length += len(resp.normal_message)
                if resp.function_call is not None:
                    resp_tmp = resp
                    function_call = resp.function_call
            if randomad.enabled:
                for word in randomad.generate_ad():
                    normal_message += word
            record.success = True
            return normal_message, function_call, resp_tmp, None
        except exceptions.QueryHandlingError as e:
            record.error = e
            record.success = False
            return None, None, None, e
        except Exception as e:
            record.error = e
            record.success = False
            logging.warning("Exception should be processed by adapter but caught by forward manager:" + str(e))
            logging.error(traceback.format_exc())
            return None, None, None, e
        finally:
            record.commit()

    @staticmethod
    def _sse_response(events: list) -> quart.Response:
        async def _gen():
            for ev in events:
                yield ev
        headers = {
            "Content-Type": "text/event-stream",
            "Cache-Control": "no-cache",
            "X-Accel-Buffering": "no",
        }
        return quart.Response(_gen(), mimetype="text/event-stream", headers=headers)

    async def _protocol_query(
        self,
        chan: channel.Channel,
        req: request.Request,
        resp_id: str,
        proto: str,
    ) -> quart.Response:
        """Anthropic Messages / OpenAI Responses 的统一入口。"""
        record = evaluation.Record()
        record.stream = req.stream
        chan.eval.add_record(record)

        before = time.time()
        record.start_time = before

        req_msg_total_length = 0
        for msg in req.messages:
            for k, v in msg.items():
                req_msg_total_length += len(str(k))
                req_msg_total_length += len(str(v))
        record.req_messages_length = req_msg_total_length

        normal_message, function_call, resp_tmp, err = await self._collect(chan, req, record, before)

        if err is not None:
            status = getattr(err, "status_code", 500) or 500
            code = getattr(err, "code", None) or "error"
            message = getattr(err, "message", None) or str(err)
            etype = getattr(err, "type", None)
            body, status = protocolmod.error_payload(proto, status, code, message, etype)
            return quart.jsonify(body), status

        prompt_tokens = chan.count_tokens(req.model, req.messages)
        completion_tokens = chan.count_tokens(
            req.model,
            [{"role": "assistant", "content": normal_message}],
        )

        if proto == protocolmod.PROTOCOL_ANTHROPIC:
            if req.stream:
                events = protocolmod.anthropic_stream_events(
                    resp_id, req.model, normal_message, function_call, prompt_tokens, completion_tokens
                )
                return self._sse_response(events)
            return quart.jsonify(protocolmod.anthropic_response(
                resp_id, req.model, normal_message, function_call, prompt_tokens, completion_tokens
            ))

        # OpenAI Responses
        if req.stream:
            events = protocolmod.responses_stream_events(
                resp_id, req.model, normal_message, function_call, prompt_tokens, completion_tokens
            )
            return self._sse_response(events)
        return quart.jsonify(protocolmod.responses_response(
            resp_id, req.model, normal_message, function_call, prompt_tokens, completion_tokens
        ))

    async def __stream_query(
        self,
        chan: channel.Channel,
        req: request.Request,
        resp_id: str,
    ) -> quart.Response:
        record: evaluation.Record = evaluation.Record()
        record.stream = True
        chan.eval.add_record(record)
        
        before = time.time()
        
        record.start_time = before
        
        # calc req msg total length
        req_msg_total_length = 0
        
        for msg in req.messages:
            for k, v in msg.items():
                req_msg_total_length += len(str(k))
                req_msg_total_length += len(str(v))
                
        record.req_messages_length = req_msg_total_length
        
        t = int(time.time())
        async def _gen():
            try:
                async for resp in chan.adapter.query(req):
                    
                    if record.latency < 0:
                        record.latency = time.time() - before
                    
                    if (resp.normal_message is None or len(resp.normal_message) == 0) and resp.finish_reason == response.FinishReason.NULL and resp.function_call is None:
                        continue

                    if resp.normal_message:
                        record.resp_message_length += len(resp.normal_message)

                    logging.debug("resp: {}".format(resp))
                    
                    yield "data: {}\n\n".format(json.dumps({
                        "id": "chatcmpl-"+resp_id,
                        "object": "chat.completion.chunk",
                        "created": t,
                        "model": req.model,
                        "choices": [{
                            "index": 0,
                            "delta": delta_of(resp, resp_id),
                            "finish_reason": finish_reason_value(resp)
                        }]
                    }))
                
                if randomad.enabled:
                    for word in randomad.generate_ad():
                        yield "data: {}\n\n".format(json.dumps({
                            "id": "chatcmpl-"+resp_id,
                            "object": "chat.completion.chunk",
                            "created": t,
                            "model": req.model,
                            "choices": [{
                                "index": 0,
                                "delta": {
                                    "content": word,
                                },
                                "finish_reason": response.FinishReason.NULL.value
                            }]
                        }))
                        
                record.success = True
                
                yield "data: [DONE]\n\n"
            except exceptions.QueryHandlingError as e:
                
                record.error = e
                record.success = False
                
                yield "data: {}\n\ndata: [DONE]\n\n".format(json.dumps({
                    "error": {
                        "code": e.code,
                        "message": e.message,
                        "type": e.type,
                        "param": e.param,
                    }
                }))
            except Exception as e:
                
                record.error = e
                record.success = False
                
                logging.warning("Exception should be processed by adapter but caught by forward manager:")
                logging.error(traceback.format_exc())
            finally:
                record.commit()
        
        spent_ms = int((time.time() - before)*1000)
        
        headers = {
            "Content-Type": "text/event-stream",
            "Transfer-Encoding": "chunked",
            "Connection": "keep-alive",
            "openai-processing-ms": str(spent_ms),
            "openai-version": "2020-10-01",
            "Cache-Control": "no-cache",
            "X-Accel-Buffering": "no",
        }
        
        return quart.Response(
            _gen(),
            mimetype="text/event-stream",
            headers=headers,
        )
    
    async def __non_stream_query(
        self,
        chan: channel.Channel,
        req: request.Request,
        resp_id: str,
    ) -> quart.Response:
        
        record = evaluation.Record()
        record.stream = False
        
        chan.eval.add_record(record)
        
        before = time.time()
        
        record.start_time = before
        
        # calc req msg total length
        req_msg_total_length = 0
        
        for msg in req.messages:
            for k, v in msg.items():
                req_msg_total_length += len(str(k))
                req_msg_total_length += len(str(v))
        
        record.req_messages_length = req_msg_total_length
        
        normal_message = ""
        
        resp_tmp: response.Response = None
        function_call: response.FunctionCall = None
        
        try:
        
            async for resp in chan.adapter.query(req):
                if record.latency < 0:
                    record.latency = time.time() - before

                logging.debug("resp: {}".format(resp))

                if resp.normal_message is not None:
                    resp_tmp = resp
                    normal_message += resp.normal_message
                    record.resp_message_length += len(resp.normal_message)

                if resp.function_call is not None:
                    resp_tmp = resp
                    function_call = resp.function_call

            if randomad.enabled:
                for word in randomad.generate_ad():
                    normal_message += word
            
            record.success = True

        except exceptions.QueryHandlingError as e:
            record.error = e
            record.success = False
            
            # check for custom error raised by adapter
            return quart.jsonify({
                "error": {
                    "code": e.code,
                    "message": e.message,
                    "type": e.type,
                    "param": e.param,
                }
            }), e.status_code
        except Exception as e:
            record.error = e
            record.success = False
            
            logging.warning("Exception should be processed by adapter but caught by forward manager:"+str(e))
            # check for other error
            return quart.jsonify({
                "error": {
                    "code": 500,
                    "message": str(e),
                    "type": "internal",
                    "param": None,
                }
            }), 500
        finally:
            record.commit()
                
        spent_ms = int((time.time() - before)*1000)
        
        prompt_tokens = chan.count_tokens(req.model, req.messages)
        completion_tokens = chan.count_tokens(
            req.model,
            [{
                "role": "assistant",
                "content": normal_message,
            }]
        )
        
        result = {
            "id": "chatcmpl-"+resp_id,
            "object": "chat.completion",
            "created": int(time.time()),
            "model": req.model,
            "choices": [
                {
                    "index": 0,
                    "message": build_message(normal_message, function_call, resp_id),
                    "finish_reason": finish_reason_value(resp_tmp)
                }
            ],
            "usage": {
                "prompt_tokens": prompt_tokens,
                "completion_tokens": completion_tokens,
                "total_tokens": prompt_tokens + completion_tokens,
            }
        }
        
        return quart.jsonify(result)

    async def query(
        self,
        path: str,
        req: request.Request,
        raw_data: dict,
        proto: str = protocolmod.PROTOCOL_OPENAI,
    ) -> quart.Response:
        
        id_suffix = "".join(random.choices(string.ascii_letters+string.digits, k=21))
        chan: channel.Channel = await self.chanmgr.select_channel(
            ADAPTER_PATH,
            req,
            id_suffix
        )
        
        # find model replacement
        if len(chan.model_mapping.keys()) > 0:
            if req.model in chan.model_mapping.keys():
                req.model = chan.model_mapping[req.model]

        if chan is None:
            pass
        
        resp_id = ""
        resp_id += "{}".format(chan.id).zfill(3)
        resp_id += chan.adapter.__class__.__name__[:5]
        
        resp_id += id_suffix
        
        # Anthropic 客户端只发 x-api-key,不发 Authorization;这里只是日志用的取值,不能直接 .startswith。
        auth = quart.request.headers.get("Authorization") or quart.request.headers.get("x-api-key") or ""
        if auth.startswith("Bearer "):
            auth = auth[7:]
        
        query_info_str = f"type=query, path={path}, proto={proto}, model={req.model}, id_suffix={resp_id}, channel_name={chan.name}, channel_adpater={chan.adapter.__class__.__name__}, key={auth}"
        
        logging.info(query_info_str)
        
        if proto in (protocolmod.PROTOCOL_ANTHROPIC, protocolmod.PROTOCOL_RESPONSES):
            return await self._protocol_query(chan, req, resp_id, proto)

        if req.stream:
            return await self.__stream_query(chan, req, resp_id)
        else:
            return await self.__non_stream_query(chan, req, resp_id)
