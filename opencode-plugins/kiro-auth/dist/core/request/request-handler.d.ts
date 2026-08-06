import type { AccountRepository } from '../../infrastructure/database/account-repository';
import type { AccountManager } from '../../plugin/accounts';
import type { KiroConfig } from '../../plugin/config';
type ToastFunction = (message: string, variant: 'info' | 'warning' | 'success' | 'error') => void;
export declare class RequestHandler {
    private accountManager;
    private config;
    private repository;
    private client?;
    private accountSelector;
    private tokenRefresher;
    private errorHandler;
    private responseHandler;
    private usageTracker;
    private retryStrategy;
    constructor(accountManager: AccountManager, config: KiroConfig, repository: AccountRepository, client?: any | undefined);
    handle(input: any, init: any, showToast: ToastFunction): Promise<Response>;
    private handleKiroRequest;
    private extractModel;
    private prepareSdkRequest;
    private handleSuccessfulRequest;
    private logSdkRequest;
    private logSdkResponse;
    private logSdkError;
    private triggerReauth;
    private allAccountsPermanentlyUnhealthy;
    private sleep;
}
export {};
