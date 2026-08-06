import type { AccountRepository } from '../../infrastructure/database/account-repository';
import type { AccountManager } from '../../plugin/accounts';
import type { ManagedAccount } from '../../plugin/types';
type ToastFunction = (message: string, variant: 'info' | 'warning' | 'success' | 'error') => void;
interface RequestContext {
    retry: number;
}
interface ErrorHandlerConfig {
    rate_limit_max_retries: number;
    rate_limit_retry_delay_ms: number;
}
export declare class ErrorHandler {
    private config;
    private accountManager;
    private repository;
    constructor(config: ErrorHandlerConfig, accountManager: AccountManager, repository: AccountRepository);
    handle(error: any, response: Response, account: ManagedAccount, context: RequestContext, showToast: ToastFunction): Promise<{
        shouldRetry: boolean;
        newContext?: RequestContext;
        switchAccount?: boolean;
    }>;
    handleNetworkError(error: any, context: RequestContext, showToast: ToastFunction): Promise<{
        shouldRetry: boolean;
        newContext?: RequestContext;
    }>;
    private isNetworkError;
    private sleep;
}
export {};
