import type { AuthHook } from '@opencode-ai/plugin';
import type { AccountRepository } from '../../infrastructure/database/account-repository.js';
type ToastFunction = (message: string, variant: 'info' | 'warning' | 'success' | 'error') => void;
export declare class AuthHandler {
    private config;
    private repository;
    private accountManager?;
    constructor(config: any, repository: AccountRepository);
    initialize(showToast?: ToastFunction): Promise<void>;
    private logUsageSummary;
    setAccountManager(am: any): void;
    getMethods(): AuthHook['methods'];
}
export {};
