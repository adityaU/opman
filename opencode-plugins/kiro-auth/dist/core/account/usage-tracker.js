import * as logger from '../../plugin/logger';
import { fetchUsageLimits, updateAccountQuota } from '../../plugin/usage';
export class UsageTracker {
    config;
    accountManager;
    repository;
    lastSyncTime = new Map();
    cooldownMs;
    constructor(config, accountManager, repository) {
        this.config = config;
        this.accountManager = accountManager;
        this.repository = repository;
        this.cooldownMs = config.usage_sync_cooldown_ms ?? 60000;
    }
    async syncUsage(account, auth) {
        if (!this.config.usage_tracking_enabled)
            return;
        const last = this.lastSyncTime.get(account.id) ?? 0;
        if (Date.now() - last < this.cooldownMs)
            return;
        this.lastSyncTime.set(account.id, Date.now());
        this.syncWithRetry(account, auth, 0).catch((e) => {
            logger.warn('Usage sync failed after all retries', {
                accountId: account.id,
                error: e instanceof Error ? e.message : String(e)
            });
        });
    }
    async syncWithRetry(account, auth, attempt) {
        try {
            const u = await fetchUsageLimits(auth);
            updateAccountQuota(account, u, this.accountManager);
            await this.repository.batchSave(this.accountManager.getAccounts());
        }
        catch (e) {
            if (attempt < this.config.usage_sync_max_retries) {
                await this.sleep(1000 * Math.pow(2, attempt));
                return this.syncWithRetry(account, auth, attempt + 1);
            }
            if (e.message?.includes('FEATURE_NOT_SUPPORTED')) {
                // Some IDC profiles don't support getUsageLimits; don't penalize the account.
                return;
            }
            if (e.message?.includes('403') ||
                e.message?.includes('invalid') ||
                e.message?.includes('bearer token')) {
                this.accountManager.markUnhealthy(account, e.message);
                this.repository.save(account).catch(() => { });
            }
            throw e;
        }
    }
    sleep(ms) {
        return new Promise((r) => setTimeout(r, ms));
    }
}
