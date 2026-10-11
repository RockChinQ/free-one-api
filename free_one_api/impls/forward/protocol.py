'''Protocol adapters: OpenAI Chat Completions / Responses / Anthropic Messages.

内部统一使用 entities.request.Request 与 entities.response.Response,
本模块只负责「外部协议 <-> 内部结构」的转换与输出拼装。
'''
import json
import time
import uuid

NL = chr(10)

PROTOCOL_OPENAI = "openai"
PROTOCOL_RESPONSES = "responses"
PROTOCOL_ANTHROPIC = "anthropic"


def new_id(prefix):
    return prefix + uuid.uuid4().hex[:24]


def sse(data, event=None):
    body = json.dumps(data, ensure_ascii=False)
    if event:
        return 'event: ' + event + NL + 'data: ' + body + NL + NL
    return 'data: ' + body + NL + NL


def text_of_content(content):
    if content is None:
        return ''
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts = []
        for item in content:
            if isinstance(item, str):
                parts.append(item)
            elif isinstance(item, dict):
                t = item.get('type')
                if t in ('text', 'input_text', 'output_text'):
                    parts.append(item.get('text') or '')
                elif t == 'tool_result':
                    parts.append(text_of_content(item.get('content')))
                elif isinstance(item.get('text'), str):
                    parts.append(item['text'])
        return ''.join(parts)
    return str(content)


def arguments_str(arguments):
    if isinstance(arguments, str):
        return arguments
    return json.dumps(arguments if arguments is not None else {}, ensure_ascii=False)


def finish_name(finish_reason):
    value = getattr(finish_reason, 'value', finish_reason)
    if value == 'function_call':
        return 'tool_calls'
    if value is None:
        return 'stop'
    return value


def error_payload(protocol, status, code, message, type_=None):
    if protocol == 'anthropic':
        return {'type': 'error', 'error': {'type': type_ or 'api_error', 'message': message}}, status
    return {'error': {'code': code, 'message': message, 'type': type_ or 'api_error', 'param': None}}, status


# ---------------- Anthropic Messages ----------------

def parse_anthropic_request(data):
    model = data.get('model') or ''
    messages = []
    system = data.get('system')
    if system:
        messages.append({'role': 'system', 'content': text_of_content(system)})
    for msg in data.get('messages') or []:
        if not isinstance(msg, dict):
            continue
        role = msg.get('role') or 'user'
        content = msg.get('content')
        if isinstance(content, list):
            texts = []
            tool_calls = []
            tool_results = []
            for block in content:
                if isinstance(block, str):
                    texts.append(block)
                    continue
                if not isinstance(block, dict):
                    continue
                t = block.get('type')
                if t == 'text':
                    texts.append(block.get('text') or '')
                elif t == 'tool_use':
                    tool_calls.append({
                        'id': block.get('id') or new_id('call_'),
                        'type': 'function',
                        'function': {
                            'name': block.get('name') or '',
                            'arguments': arguments_str(block.get('input') or {}),
                        },
                    })
                elif t == 'tool_result':
                    tool_results.append({
                        'role': 'tool',
                        'tool_call_id': block.get('tool_use_id') or '',
                        'content': text_of_content(block.get('content')),
                    })
            joined = ''.join(texts)
            if tool_results:
                messages.extend(tool_results)
                if joined:
                    messages.append({'role': role, 'content': joined})
            elif tool_calls:
                messages.append({'role': 'assistant', 'content': joined or None, 'tool_calls': tool_calls})
            else:
                messages.append({'role': role, 'content': joined})
        else:
            messages.append({'role': role, 'content': text_of_content(content)})

    tools = None
    if data.get('tools'):
        tools = []
        for t in data['tools']:
            if not isinstance(t, dict):
                continue
            tools.append({
                'type': 'function',
                'function': {
                    'name': t.get('name') or '',
                    'description': t.get('description') or '',
                    'parameters': t.get('input_schema') or {'type': 'object', 'properties': {}},
                },
            })

    tool_choice = None
    tc = data.get('tool_choice')
    if isinstance(tc, dict):
        typ = tc.get('type')
        if typ == 'auto':
            tool_choice = 'auto'
        elif typ == 'any':
            tool_choice = 'required'
        elif typ == 'none':
            tool_choice = 'none'
        elif typ == 'tool':
            tool_choice = {'type': 'function', 'function': {'name': tc.get('name') or ''}}

    return {
        'model': model,
        'messages': messages,
        'stream': bool(data.get('stream')),
        'tools': tools,
        'tool_choice': tool_choice,
    }


def anthropic_response(resp_id, model, text, fc, prompt_tokens, completion_tokens):
    content = []
    stop_reason = 'end_turn'
    if fc is not None:
        if text:
            content.append({'type': 'text', 'text': text})
        content.append({
            'type': 'tool_use',
            'id': fc.id or new_id('toolu_'),
            'name': fc.function_name,
            'input': fc.arguments if isinstance(fc.arguments, dict) else {},
        })
        stop_reason = 'tool_use'
    else:
        content.append({'type': 'text', 'text': text or ''})
    return {
        'id': 'msg_' + resp_id,
        'type': 'message',
        'role': 'assistant',
        'model': model,
        'content': content,
        'stop_reason': stop_reason,
        'stop_sequence': None,
        'usage': {'input_tokens': prompt_tokens, 'output_tokens': completion_tokens},
    }


def anthropic_stream_events(resp_id, model, text, fc, prompt_tokens, completion_tokens):
    events = []
    msg_id = 'msg_' + resp_id
    events.append(sse({
        'type': 'message_start',
        'message': {
            'id': msg_id, 'type': 'message', 'role': 'assistant', 'model': model,
            'content': [], 'stop_reason': None, 'stop_sequence': None,
            'usage': {'input_tokens': prompt_tokens, 'output_tokens': 0},
        },
    }, 'message_start'))
    events.append(sse({'type': 'ping'}, 'ping'))

    index = 0
    if text:
        events.append(sse({'type': 'content_block_start', 'index': index, 'content_block': {'type': 'text', 'text': ''}}, 'content_block_start'))
        events.append(sse({'type': 'content_block_delta', 'index': index, 'delta': {'type': 'text_delta', 'text': text}}, 'content_block_delta'))
        events.append(sse({'type': 'content_block_stop', 'index': index}, 'content_block_stop'))
        index += 1
    if fc is not None:
        events.append(sse({'type': 'content_block_start', 'index': index, 'content_block': {'type': 'tool_use', 'id': fc.id or new_id('toolu_'), 'name': fc.function_name, 'input': {}}}, 'content_block_start'))
        events.append(sse({'type': 'content_block_delta', 'index': index, 'delta': {'type': 'input_json_delta', 'partial_json': arguments_str(fc.arguments)}}, 'content_block_delta'))
        events.append(sse({'type': 'content_block_stop', 'index': index}, 'content_block_stop'))
    if not text and fc is None:
        events.append(sse({'type': 'content_block_start', 'index': 0, 'content_block': {'type': 'text', 'text': ''}}, 'content_block_start'))
        events.append(sse({'type': 'content_block_stop', 'index': 0}, 'content_block_stop'))

    events.append(sse({
        'type': 'message_delta',
        'delta': {'stop_reason': 'tool_use' if fc is not None else 'end_turn', 'stop_sequence': None},
        'usage': {'output_tokens': completion_tokens},
    }, 'message_delta'))
    events.append(sse({'type': 'message_stop'}, 'message_stop'))
    return events


# ---------------- OpenAI Responses ----------------

def parse_responses_request(data):
    model = data.get('model') or ''
    messages = []
    instructions = data.get('instructions')
    if instructions:
        messages.append({'role': 'system', 'content': text_of_content(instructions)})
    inp = data.get('input')
    if isinstance(inp, str):
        messages.append({'role': 'user', 'content': inp})
    elif isinstance(inp, list):
        for item in inp:
            if isinstance(item, str):
                messages.append({'role': 'user', 'content': item})
                continue
            if not isinstance(item, dict):
                continue
            typ = item.get('type')
            if typ == 'function_call':
                messages.append({
                    'role': 'assistant',
                    'content': None,
                    'tool_calls': [{
                        'id': item.get('call_id') or item.get('id') or new_id('call_'),
                        'type': 'function',
                        'function': {'name': item.get('name') or '', 'arguments': item.get('arguments') or '{}'},
                    }],
                })
            elif typ == 'function_call_output':
                messages.append({'role': 'tool', 'tool_call_id': item.get('call_id') or '', 'content': text_of_content(item.get('output'))})
            else:
                role = item.get('role') or 'user'
                messages.append({'role': role, 'content': text_of_content(item.get('content'))})

    tools = None
    if data.get('tools'):
        tools = []
        for t in data['tools']:
            if not isinstance(t, dict):
                continue
            if t.get('type') != 'function':
                continue
            if isinstance(t.get('function'), dict):
                tools.append(t)
            else:
                tools.append({
                    'type': 'function',
                    'function': {
                        'name': t.get('name') or '',
                        'description': t.get('description') or '',
                        'parameters': t.get('parameters') or {'type': 'object', 'properties': {}},
                    },
                })

    tool_choice = data.get('tool_choice')
    if isinstance(tool_choice, dict) and 'function' not in tool_choice and tool_choice.get('name'):
        tool_choice = {'type': 'function', 'function': {'name': tool_choice['name']}}

    return {
        'model': model,
        'messages': messages,
        'stream': bool(data.get('stream')),
        'tools': tools,
        'tool_choice': tool_choice,
    }


def responses_response(resp_id, model, text, fc, prompt_tokens, completion_tokens):
    output = []
    if fc is not None:
        output.append({
            'type': 'function_call',
            'id': new_id('fc_'),
            'call_id': fc.id or new_id('call_'),
            'name': fc.function_name,
            'arguments': arguments_str(fc.arguments),
            'status': 'completed',
        })
    if text or fc is None:
        output.append({
            'type': 'message',
            'id': new_id('msg_'),
            'status': 'completed',
            'role': 'assistant',
            'content': [{'type': 'output_text', 'text': text or '', 'annotations': []}],
        })
    return {
        'id': 'resp_' + resp_id,
        'object': 'response',
        'created_at': int(time.time()),
        'status': 'completed',
        'model': model,
        'output': output,
        'output_text': text or '',
        'parallel_tool_calls': True,
        'error': None,
        'incomplete_details': None,
        'usage': {
            'input_tokens': prompt_tokens,
            'output_tokens': completion_tokens,
            'total_tokens': prompt_tokens + completion_tokens,
        },
    }


def responses_stream_events(resp_id, model, text, fc, prompt_tokens, completion_tokens):
    events = []
    resp_obj_id = 'resp_' + resp_id
    created = int(time.time())
    shell = {
        'id': resp_obj_id, 'object': 'response', 'created_at': created,
        'status': 'in_progress', 'model': model, 'output': [],
        'parallel_tool_calls': True, 'error': None, 'incomplete_details': None,
    }
    events.append(sse({'type': 'response.created', 'response': dict(shell)}, 'response.created'))
    events.append(sse({'type': 'response.in_progress', 'response': dict(shell)}, 'response.in_progress'))

    final_output = []
    if fc is not None:
        item = {
            'type': 'function_call', 'id': new_id('fc_'), 'call_id': fc.id or new_id('call_'),
            'name': fc.function_name, 'arguments': arguments_str(fc.arguments), 'status': 'completed',
        }
        started = dict(item)
        started['status'] = 'in_progress'
        events.append(sse({'type': 'response.output_item.added', 'output_index': 0, 'item': started}, 'response.output_item.added'))
        events.append(sse({'type': 'response.function_call_arguments.delta', 'item_id': item['id'], 'output_index': 0, 'delta': item['arguments']}, 'response.function_call_arguments.delta'))
        events.append(sse({'type': 'response.function_call_arguments.done', 'item_id': item['id'], 'output_index': 0, 'arguments': item['arguments']}, 'response.function_call_arguments.done'))
        events.append(sse({'type': 'response.output_item.done', 'output_index': 0, 'item': item}, 'response.output_item.done'))
        final_output.append(item)

    if text or fc is None:
        idx = len(final_output)
        item_id = new_id('msg_')
        item = {
            'type': 'message', 'id': item_id, 'status': 'completed', 'role': 'assistant',
            'content': [{'type': 'output_text', 'text': text or '', 'annotations': []}],
        }
        started = {'type': 'message', 'id': item_id, 'status': 'in_progress', 'role': 'assistant', 'content': []}
        events.append(sse({'type': 'response.output_item.added', 'output_index': idx, 'item': started}, 'response.output_item.added'))
        events.append(sse({'type': 'response.content_part.added', 'item_id': item_id, 'output_index': idx, 'content_index': 0, 'part': {'type': 'output_text', 'text': '', 'annotations': []}}, 'response.content_part.added'))
        if text:
            events.append(sse({'type': 'response.output_text.delta', 'item_id': item_id, 'output_index': idx, 'content_index': 0, 'delta': text}, 'response.output_text.delta'))
        events.append(sse({'type': 'response.output_text.done', 'item_id': item_id, 'output_index': idx, 'content_index': 0, 'text': text or ''}, 'response.output_text.done'))
        events.append(sse({'type': 'response.content_part.done', 'item_id': item_id, 'output_index': idx, 'content_index': 0, 'part': {'type': 'output_text', 'text': text or '', 'annotations': []}}, 'response.content_part.done'))
        events.append(sse({'type': 'response.output_item.done', 'output_index': idx, 'item': item}, 'response.output_item.done'))
        final_output.append(item)

    done = dict(shell)
    done['status'] = 'completed'
    done['output'] = final_output
    done['output_text'] = text or ''
    done['usage'] = {
        'input_tokens': prompt_tokens,
        'output_tokens': completion_tokens,
        'total_tokens': prompt_tokens + completion_tokens,
    }
    events.append(sse({'type': 'response.completed', 'response': done}, 'response.completed'))
    return events
