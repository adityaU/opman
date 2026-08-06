import { parseBracketToolCalls } from '../../infrastructure/transformers/tool-call-parser.js';
import { getContextWindowSize } from '../models.js';
import { estimateTokens } from '../response.js';
import { convertToOpenAI } from './openai-converter.js';
import { findRealTag } from './stream-parser.js';
import { createTextDeltaEvents, createThinkingDeltaEvents, stopBlock } from './stream-state.js';
import { THINKING_END_TAG, THINKING_START_TAG } from './types.js';
export async function* transformSdkStream(sdkResponse, model, conversationId) {
    const thinkingRequested = true;
    const streamState = {
        thinkingRequested,
        buffer: '',
        inThinking: false,
        thinkingExtracted: false,
        thinkingBlockIndex: null,
        textBlockIndex: null,
        nextBlockIndex: 0,
        stoppedBlocks: new Set()
    };
    let totalContent = '';
    let textOnlyContent = '';
    let outputTokens = 0;
    let inputTokens = 0;
    let contextUsagePercentage = null;
    const toolCalls = [];
    let currentToolCall = null;
    const eventStream = sdkResponse.generateAssistantResponseResponse;
    if (!eventStream) {
        throw new Error('SDK response has no event stream');
    }
    try {
        for await (const event of eventStream) {
            if (event.assistantResponseEvent?.content) {
                const text = event.assistantResponseEvent.content;
                totalContent += text;
                textOnlyContent += text;
                if (!thinkingRequested) {
                    for (const ev of createTextDeltaEvents(text, streamState)) {
                        yield convertToOpenAI(ev, conversationId, model);
                    }
                    continue;
                }
                streamState.buffer += text;
                const deltaEvents = [];
                while (streamState.buffer.length > 0) {
                    if (!streamState.inThinking && !streamState.thinkingExtracted) {
                        const startPos = findRealTag(streamState.buffer, THINKING_START_TAG);
                        if (startPos !== -1) {
                            const before = streamState.buffer.slice(0, startPos);
                            if (before) {
                                deltaEvents.push(...createTextDeltaEvents(before, streamState));
                            }
                            streamState.buffer = streamState.buffer.slice(startPos + THINKING_START_TAG.length);
                            streamState.inThinking = true;
                            continue;
                        }
                        const safeLen = Math.max(0, streamState.buffer.length - THINKING_START_TAG.length);
                        if (safeLen > 0) {
                            const safeText = streamState.buffer.slice(0, safeLen);
                            if (safeText) {
                                deltaEvents.push(...createTextDeltaEvents(safeText, streamState));
                            }
                            streamState.buffer = streamState.buffer.slice(safeLen);
                        }
                        break;
                    }
                    if (streamState.inThinking) {
                        const endPos = findRealTag(streamState.buffer, THINKING_END_TAG);
                        if (endPos !== -1) {
                            const thinkingPart = streamState.buffer.slice(0, endPos);
                            if (thinkingPart) {
                                deltaEvents.push(...createThinkingDeltaEvents(thinkingPart, streamState));
                            }
                            streamState.buffer = streamState.buffer.slice(endPos + THINKING_END_TAG.length);
                            streamState.inThinking = false;
                            streamState.thinkingExtracted = true;
                            deltaEvents.push(...createThinkingDeltaEvents('', streamState));
                            deltaEvents.push(...stopBlock(streamState.thinkingBlockIndex, streamState));
                            if (streamState.buffer.startsWith('\n\n')) {
                                streamState.buffer = streamState.buffer.slice(2);
                            }
                            continue;
                        }
                        const safeLen = Math.max(0, streamState.buffer.length - THINKING_END_TAG.length);
                        if (safeLen > 0) {
                            const safeThinking = streamState.buffer.slice(0, safeLen);
                            if (safeThinking) {
                                deltaEvents.push(...createThinkingDeltaEvents(safeThinking, streamState));
                            }
                            streamState.buffer = streamState.buffer.slice(safeLen);
                        }
                        break;
                    }
                    if (streamState.thinkingExtracted) {
                        const rest = streamState.buffer;
                        streamState.buffer = '';
                        if (rest) {
                            deltaEvents.push(...createTextDeltaEvents(rest, streamState));
                        }
                        break;
                    }
                }
                for (const ev of deltaEvents) {
                    yield convertToOpenAI(ev, conversationId, model);
                }
            }
            else if (event.toolUseEvent) {
                const tc = event.toolUseEvent;
                if (tc.name)
                    totalContent += tc.name;
                if (tc.input)
                    totalContent += tc.input;
                if (tc.name && tc.toolUseId) {
                    if (currentToolCall && currentToolCall.toolUseId === tc.toolUseId) {
                        currentToolCall.input += tc.input || '';
                    }
                    else {
                        if (currentToolCall)
                            toolCalls.push(currentToolCall);
                        currentToolCall = {
                            toolUseId: tc.toolUseId,
                            name: tc.name,
                            input: tc.input || ''
                        };
                    }
                    if (tc.stop && currentToolCall) {
                        toolCalls.push(currentToolCall);
                        currentToolCall = null;
                    }
                }
            }
            else if (event.metadataEvent) {
                if (event.metadataEvent.contextUsagePercentage) {
                    contextUsagePercentage = event.metadataEvent.contextUsagePercentage;
                }
            }
            else if (event.contextUsageEvent) {
                const cue = event.contextUsageEvent;
                if (cue.contextUsagePercentage) {
                    contextUsagePercentage = cue.contextUsagePercentage;
                }
            }
        }
        if (currentToolCall) {
            toolCalls.push(currentToolCall);
            currentToolCall = null;
        }
        if (thinkingRequested && streamState.buffer) {
            if (streamState.inThinking) {
                for (const ev of createThinkingDeltaEvents(streamState.buffer, streamState))
                    yield convertToOpenAI(ev, conversationId, model);
                streamState.buffer = '';
                for (const ev of createThinkingDeltaEvents('', streamState))
                    yield convertToOpenAI(ev, conversationId, model);
                for (const ev of stopBlock(streamState.thinkingBlockIndex, streamState))
                    yield convertToOpenAI(ev, conversationId, model);
            }
            else {
                for (const ev of createTextDeltaEvents(streamState.buffer, streamState))
                    yield convertToOpenAI(ev, conversationId, model);
                streamState.buffer = '';
            }
        }
        for (const ev of stopBlock(streamState.textBlockIndex, streamState))
            yield convertToOpenAI(ev, conversationId, model);
        const bracketToolCalls = parseBracketToolCalls(totalContent);
        if (bracketToolCalls.length > 0) {
            for (const btc of bracketToolCalls) {
                toolCalls.push({
                    toolUseId: btc.toolUseId,
                    name: btc.name,
                    input: typeof btc.input === 'string' ? btc.input : JSON.stringify(btc.input)
                });
            }
        }
        if (toolCalls.length > 0) {
            const baseIndex = streamState.nextBlockIndex;
            for (let i = 0; i < toolCalls.length; i++) {
                const tc = toolCalls[i];
                if (!tc)
                    continue;
                const blockIndex = baseIndex + i;
                yield convertToOpenAI({
                    type: 'content_block_start',
                    index: blockIndex,
                    content_block: {
                        type: 'tool_use',
                        id: tc.toolUseId,
                        name: tc.name,
                        input: {}
                    }
                }, conversationId, model);
                let inputJson;
                try {
                    const parsed = JSON.parse(tc.input);
                    inputJson = JSON.stringify(parsed);
                }
                catch (e) {
                    inputJson = tc.input;
                }
                yield convertToOpenAI({
                    type: 'content_block_delta',
                    index: blockIndex,
                    delta: {
                        type: 'input_json_delta',
                        partial_json: inputJson
                    }
                }, conversationId, model);
                yield convertToOpenAI({ type: 'content_block_stop', index: blockIndex }, conversationId, model);
            }
        }
        outputTokens = estimateTokens(textOnlyContent);
        if (contextUsagePercentage !== null && contextUsagePercentage > 0) {
            const contextWindow = getContextWindowSize(model);
            const totalTokens = Math.round((contextWindow * contextUsagePercentage) / 100);
            inputTokens = Math.max(0, totalTokens - outputTokens);
        }
        yield convertToOpenAI({
            type: 'message_delta',
            delta: { stop_reason: toolCalls.length > 0 ? 'tool_use' : 'end_turn' },
            usage: {
                input_tokens: inputTokens,
                output_tokens: outputTokens,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0
            }
        }, conversationId, model);
        yield convertToOpenAI({ type: 'message_stop' }, conversationId, model);
    }
    catch (e) {
        throw e;
    }
}
