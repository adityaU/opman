import { KiroAuthDetails, ManagedAccount } from './types';
export declare function fetchUsageLimits(auth: KiroAuthDetails): Promise<any>;
export declare function updateAccountQuota(account: ManagedAccount, usage: any, accountManager?: any): void;
