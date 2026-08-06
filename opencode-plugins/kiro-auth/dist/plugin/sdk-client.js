import { CodeWhispererStreamingClient } from '@aws/codewhisperer-streaming-client';
import { KIRO_CONSTANTS } from '../constants.js';
const clientCache = new Map();
export function createSdkClient(auth, region) {
    const cacheKey = `${region}:${auth.email || 'default'}`;
    const cached = clientCache.get(cacheKey);
    if (cached && cached.token === auth.access) {
        return cached.client;
    }
    const token = auth.access;
    const client = new CodeWhispererStreamingClient({
        region,
        endpoint: `https://q.${region}.amazonaws.com`,
        token: () => Promise.resolve({ token }),
        maxAttempts: 1,
        customUserAgent: [[KIRO_CONSTANTS.USER_AGENT]]
    });
    client.middlewareStack.add((next) => async (args) => {
        if (!args.request.headers['x-amzn-kiro-agent-mode']) {
            args.request.headers['x-amzn-kiro-agent-mode'] = 'vibe';
        }
        return next(args);
    }, { step: 'build', name: 'addKiroHeaders' });
    clientCache.set(cacheKey, { client, token });
    return client;
}
export function clearSdkClientCache() {
    for (const entry of clientCache.values()) {
        entry.client.destroy();
    }
    clientCache.clear();
}
