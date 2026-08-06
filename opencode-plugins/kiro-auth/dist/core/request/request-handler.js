import { GenerateAssistantResponseCommand } from '@aws/codewhisperer-streaming-client';
import { isPermanentError } from '../../plugin/health';
import * as logger from '../../plugin/logger';
import { transformToSdkRequest } from '../../plugin/request';
import { createSdkClient } from '../../plugin/sdk-client';
import { syncFromKiroCli } from '../../plugin/sync/kiro-cli';
import { AccountSelector } from '../account/account-selector';
import { UsageTracker } from '../account/usage-tracker';
import { TokenRefresher } from '../auth/token-refresher';
import { ErrorHandler } from './error-handler';
import { ResponseHandler } from './response-handler';
import { RetryStrategy } from './retry-strategy';
const KIRO_API_PATTERN = /^(https?:\/\/)?q\.[a-z0-9-]+\.amazonaws\.com/;
export class RequestHandler {
    accountManager;
    config;
    repository;
    client;
    accountSelector;
    tokenRefresher;
    errorHandler;
    responseHandler;
    usageTracker;
    retryStrategy;
    constructor(accountManager, config, repository, client) {
        this.accountManager = accountManager;
        this.config = config;
        this.repository = repository;
        this.client = client;
        this.accountSelector = new AccountSelector(accountManager, config, syncFromKiroCli, repository);
        this.tokenRefresher = new TokenRefresher(config, accountManager, syncFromKiroCli, repository);
        this.errorHandler = new ErrorHandler(config, accountManager, repository);
        this.responseHandler = new ResponseHandler();
        this.usageTracker = new UsageTracker(config, accountManager, repository);
        this.retryStrategy = new RetryStrategy(config);
    }
    async handle(input, init, showToast) {
        const url = typeof input === 'string' ? input : input.url;
        if (!KIRO_API_PATTERN.test(url)) {
            return fetch(input, init);
        }
        return this.handleKiroRequest(url, init, showToast);
    }
    async handleKiroRequest(url, init, showToast) {
        const body = init?.body ? JSON.parse(init.body) : {};
        const model = this.extractModel(url) || body.model || 'claude-sonnet-4-5';
        const think = model.endsWith('-thinking') || !!body.providerOptions?.thinkingConfig;
        const budget = body.providerOptions?.thinkingConfig?.thinkingBudget || 20000;
        let retry = 0;
        let consecutiveNullAccounts = 0;
        const retryContext = this.retryStrategy.createContext();
        while (true) {
            const check = this.retryStrategy.shouldContinue(retryContext);
            if (!check.canContinue) {
                throw new Error(check.error);
            }
            if (this.allAccountsPermanentlyUnhealthy()) {
                const reauthed = await this.triggerReauth(showToast);
                if (!reauthed) {
                    throw new Error('All accounts are permanently unhealthy. Please re-authenticate.');
                }
                continue;
            }
            let acc = await this.accountSelector.selectHealthyAccount(showToast);
            if (!acc) {
                consecutiveNullAccounts++;
                const backoffDelay = Math.min(1000 * Math.pow(2, consecutiveNullAccounts - 1), 10000);
                await this.sleep(backoffDelay);
                continue;
            }
            consecutiveNullAccounts = 0;
            const auth = this.accountManager.toAuthDetails(acc);
            const tokenResult = await this.tokenRefresher.refreshIfNeeded(acc, auth, showToast);
            if (tokenResult.shouldContinue) {
                acc = tokenResult.account;
                await this.sleep(500);
                continue;
            }
            const sdkPrep = this.prepareSdkRequest(init?.body, model, auth, think, budget, showToast);
            const apiTimestamp = this.config.enable_log_api_request ? logger.getTimestamp() : null;
            if (apiTimestamp) {
                this.logSdkRequest(sdkPrep, acc, apiTimestamp);
            }
            try {
                const client = createSdkClient(auth, sdkPrep.region);
                const command = new GenerateAssistantResponseCommand({
                    conversationState: sdkPrep.conversationState,
                    profileArn: sdkPrep.profileArn
                });
                const sdkResponse = await client.send(command);
                if (apiTimestamp) {
                    this.logSdkResponse(sdkPrep, apiTimestamp);
                }
                this.handleSuccessfulRequest(acc);
                this.usageTracker.syncUsage(acc, auth);
                return await this.responseHandler.handleSdkSuccess(sdkResponse, model, sdkPrep.conversationId, sdkPrep.streaming);
            }
            catch (e) {
                const httpStatus = e?.$metadata?.httpStatusCode;
                if (httpStatus) {
                    if (apiTimestamp) {
                        this.logSdkError(sdkPrep, e, acc, apiTimestamp);
                    }
                    const mockResponse = new Response(JSON.stringify({ message: e.message, __type: e.name }), {
                        status: httpStatus,
                        statusText: e.name || 'Error',
                        headers: { 'Content-Type': 'application/json' }
                    });
                    const errorResult = await this.errorHandler.handle(e, mockResponse, acc, { retry }, showToast);
                    if (errorResult.shouldRetry) {
                        if (errorResult.newContext) {
                            retry = errorResult.newContext.retry;
                        }
                        if (errorResult.switchAccount) {
                            continue;
                        }
                        continue;
                    }
                    throw new Error(`Kiro Error: ${httpStatus}`);
                }
                const networkResult = await this.errorHandler.handleNetworkError(e, { retry }, showToast);
                if (networkResult.shouldRetry) {
                    if (networkResult.newContext) {
                        retry = networkResult.newContext.retry;
                    }
                    continue;
                }
                throw e;
            }
        }
    }
    extractModel(url) {
        return url.match(/models\/([^/:]+)/)?.[1] || null;
    }
    prepareSdkRequest(body, model, auth, think, budget, showToast) {
        return transformToSdkRequest(body, model, auth, think, budget, showToast);
    }
    handleSuccessfulRequest(acc) {
        if (acc.failCount && acc.failCount > 0) {
            if (!isPermanentError(acc.unhealthyReason)) {
                acc.failCount = 0;
                acc.isHealthy = true;
                delete acc.unhealthyReason;
                delete acc.recoveryTime;
                this.repository.save(acc).catch(() => { });
            }
        }
    }
    logSdkRequest(prep, acc, timestamp) {
        logger.logApiRequest({
            url: `https://q.${prep.region}.amazonaws.com/generateAssistantResponse`,
            method: 'POST',
            headers: { 'x-amzn-kiro-agent-mode': 'vibe' },
            body: {
                conversationState: {
                    chatTriggerType: prep.conversationState.chatTriggerType,
                    conversationId: prep.conversationState.conversationId,
                    historyLength: prep.conversationState.history?.length || 0,
                    currentMessage: prep.conversationState.currentMessage
                },
                profileArn: prep.profileArn
            },
            conversationId: prep.conversationId,
            model: prep.effectiveModel,
            email: acc.email
        }, timestamp);
    }
    logSdkResponse(prep, timestamp) {
        logger.logApiResponse({
            status: 200,
            statusText: 'OK',
            headers: {},
            conversationId: prep.conversationId,
            model: prep.effectiveModel
        }, timestamp);
    }
    logSdkError(prep, error, acc, apiTimestamp) {
        const status = error?.$metadata?.httpStatusCode || 0;
        const rData = {
            status,
            statusText: error?.name || 'Error',
            headers: {},
            error: `Kiro Error: ${status} - ${error?.message || 'Unknown'}`,
            conversationId: prep.conversationId,
            model: prep.effectiveModel
        };
        if (!this.config.enable_log_api_request) {
            logger.logApiError({
                url: `https://q.${prep.region}.amazonaws.com/generateAssistantResponse`,
                method: 'POST',
                headers: {},
                body: null,
                conversationId: prep.conversationId,
                model: prep.effectiveModel,
                email: acc.email
            }, rData, logger.getTimestamp());
        }
        else {
            logger.logApiResponse(rData, apiTimestamp);
        }
    }
    async triggerReauth(showToast) {
        if (!this.client)
            return false;
        try {
            showToast('Session expired. Re-authenticating...', 'warning');
            await this.client.provider.oauth.authorize({
                path: { id: 'kiro' },
                body: { method: 0 }
            });
            // Sync fresh tokens from CLI after re-auth
            await syncFromKiroCli();
            this.repository.invalidateCache();
            const accounts = await this.repository.findAll();
            for (const acc of accounts) {
                this.accountManager.addAccount(acc);
            }
            showToast('Re-authentication successful.', 'success');
            return true;
        }
        catch (e) {
            logger.error('Re-auth failed', e instanceof Error ? e : new Error(String(e)));
            return false;
        }
    }
    allAccountsPermanentlyUnhealthy() {
        const accounts = this.accountManager.getAccounts();
        if (accounts.length === 0) {
            return false;
        }
        return accounts.every((acc) => !acc.isHealthy && isPermanentError(acc.unhealthyReason));
    }
    sleep(ms) {
        return new Promise((resolve) => setTimeout(resolve, ms));
    }
}
