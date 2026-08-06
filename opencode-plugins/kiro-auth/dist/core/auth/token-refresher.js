import { accessTokenExpired } from '../../kiro/auth';
import { KiroTokenRefreshError } from '../../plugin/errors';
import * as logger from '../../plugin/logger';
import { refreshAccessToken } from '../../plugin/token';
export class TokenRefresher {
    config;
    accountManager;
    syncFromKiroCli;
    repository;
    constructor(config, accountManager, syncFromKiroCli, repository) {
        this.config = config;
        this.accountManager = accountManager;
        this.syncFromKiroCli = syncFromKiroCli;
        this.repository = repository;
    }
    async refreshIfNeeded(account, auth, showToast) {
        if (!accessTokenExpired(auth, this.config.token_expiry_buffer_ms)) {
            return { account, shouldContinue: false };
        }
        try {
            const newAuth = await refreshAccessToken(auth);
            this.accountManager.updateFromAuth(account, newAuth);
            await this.repository.batchSave(this.accountManager.getAccounts());
            return { account, shouldContinue: false };
        }
        catch (e) {
            return await this.handleRefreshError(e, account, showToast);
        }
    }
    async handleRefreshError(error, account, showToast) {
        logger.error('Token refresh failed', {
            email: account.email,
            code: error instanceof KiroTokenRefreshError ? error.code : undefined,
            message: error instanceof Error ? error.message : String(error)
        });
        if (this.config.auto_sync_kiro_cli) {
            await this.syncFromKiroCli();
        }
        this.repository.invalidateCache();
        const accounts = await this.repository.findAll();
        const stillAcc = accounts.find((a) => a.id === account.id);
        if (stillAcc &&
            !accessTokenExpired(this.accountManager.toAuthDetails(stillAcc), this.config.token_expiry_buffer_ms)) {
            showToast('Credentials recovered from Kiro CLI sync.', 'info');
            return { account: stillAcc, shouldContinue: true };
        }
        if (error instanceof KiroTokenRefreshError &&
            (error.code === 'ExpiredTokenException' ||
                error.code === 'InvalidTokenException' ||
                error.code === 'HTTP_401' ||
                error.code === 'HTTP_403' ||
                error.message.includes('Invalid refresh token provided') ||
                error.message.includes('Invalid grant provided'))) {
            this.accountManager.markUnhealthy(account, error.message);
            await this.repository.batchSave(this.accountManager.getAccounts());
            return { account, shouldContinue: true };
        }
        logger.error('Token refresh unrecoverable', {
            email: account.email,
            code: error instanceof KiroTokenRefreshError ? error.code : undefined,
            message: error instanceof Error ? error.message : String(error)
        });
        throw error;
    }
}
