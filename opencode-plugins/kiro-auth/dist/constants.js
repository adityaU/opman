import { RegionSchema } from './plugin/config/schema';
const VALID_REGIONS = Object.values(RegionSchema.Values);
export function isValidRegion(region) {
    return VALID_REGIONS.includes(region);
}
export function normalizeRegion(region) {
    if (!region || !isValidRegion(region)) {
        return 'us-east-1';
    }
    return region;
}
export function buildUrl(template, region) {
    const url = template.replace('{{region}}', region);
    try {
        new URL(url);
        return url;
    }
    catch {
        throw new Error(`Invalid URL generated: ${url}`);
    }
}
export function extractRegionFromArn(arn) {
    if (!arn)
        return undefined;
    const parts = arn.split(':');
    if (parts.length < 6)
        return undefined;
    if (parts[0] !== 'arn')
        return undefined;
    const region = parts[3];
    if (typeof region !== 'string' || !region)
        return undefined;
    return isValidRegion(region) ? region : undefined;
}
export const KIRO_CONSTANTS = {
    REFRESH_URL: 'https://prod.{{region}}.auth.desktop.kiro.dev/refreshToken',
    REFRESH_IDC_URL: 'https://oidc.{{region}}.amazonaws.com/token',
    BASE_URL: 'https://q.{{region}}.amazonaws.com/generateAssistantResponse',
    USAGE_LIMITS_URL: 'https://q.{{region}}.amazonaws.com/getUsageLimits',
    DEFAULT_REGION: 'us-east-1',
    AXIOS_TIMEOUT: 120000,
    USER_AGENT: 'KiroIDE',
    SDK_VERSION: '3.738.0',
    SDK_VERSION_USAGE: '3.0.0',
    CHAT_TRIGGER_TYPE_MANUAL: 'MANUAL',
    ORIGIN_AI_EDITOR: 'AI_EDITOR'
};
export const MODEL_MAPPING = {
    'claude-haiku-4-5': 'claude-haiku-4.5',
    'claude-haiku-4-5-thinking': 'claude-haiku-4.5',
    'claude-sonnet-4-5': 'claude-sonnet-4.5',
    'claude-sonnet-4-5-thinking': 'claude-sonnet-4.5',
    'claude-sonnet-4-5-1m': 'claude-sonnet-4.5-1m',
    'claude-sonnet-4-5-1m-thinking': 'claude-sonnet-4.5-1m',
    'claude-sonnet-4-6': 'claude-sonnet-4.6',
    'claude-sonnet-4-6-thinking': 'claude-sonnet-4.6',
    'claude-sonnet-4-6-1m': 'claude-sonnet-4.6-1m',
    'claude-sonnet-4-6-1m-thinking': 'claude-sonnet-4.6-1m',
    'claude-opus-4-5': 'claude-opus-4.5',
    'claude-opus-4-5-thinking': 'claude-opus-4.5',
    'claude-opus-4-6': 'claude-opus-4.6',
    'claude-opus-4-6-thinking': 'claude-opus-4.6',
    'claude-opus-4-6-1m': 'claude-opus-4.6-1m',
    'claude-opus-4-6-1m-thinking': 'claude-opus-4.6-1m',
    'claude-opus-4-7': 'claude-opus-4.7',
    'claude-opus-4-7-thinking': 'claude-opus-4.7',
    'claude-sonnet-4': 'claude-sonnet-4',
    'claude-3-7-sonnet': 'CLAUDE_3_7_SONNET_20250219_V1_0',
    'nova-swe': 'AGI_NOVA_SWE_V1_5',
    'gpt-oss-120b': 'OPENAI_GPT_OSS_120B_1_0',
    'minimax-m2': 'MINIMAX_MINIMAX_M2',
    'kimi-k2-thinking': 'MOONSHOT_KIMI_K2_THINKING',
    auto: 'auto',
    'deepseek-3.2': 'deepseek-3.2',
    'minimax-m2.5': 'minimax-m2.5',
    'minimax-m2.1': 'minimax-m2.1',
    'qwen3-coder-next': 'qwen3-coder-next'
};
export const SUPPORTED_MODELS = Object.keys(MODEL_MAPPING);
const LONG_CONTEXT_MODELS = new Set(Object.keys(MODEL_MAPPING).filter((k) => k.includes('-1m')));
export function isLongContextModel(model) {
    return LONG_CONTEXT_MODELS.has(model);
}
export const KIRO_AUTH_SERVICE = {
    ENDPOINT: 'https://prod.{{region}}.auth.desktop.kiro.dev',
    SSO_OIDC_ENDPOINT: 'https://oidc.{{region}}.amazonaws.com',
    BUILDER_ID_START_URL: 'https://view.awsapps.com/start',
    USER_INFO_URL: 'https://view.awsapps.com/api/user/info',
    SCOPES: [
        'codewhisperer:completions',
        'codewhisperer:analysis',
        'codewhisperer:conversations',
        'codewhisperer:transformations',
        'codewhisperer:taskassist'
    ]
};
