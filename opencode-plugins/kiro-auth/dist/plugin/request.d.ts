import type { KiroAuthDetails, PreparedRequest, SdkPreparedRequest } from './types';
type ToastFunction = (message: string, variant: 'info' | 'warning' | 'success' | 'error') => void;
export declare function transformToCodeWhisperer(url: string, body: any, model: string, auth: KiroAuthDetails, think?: boolean, budget?: number): PreparedRequest;
export declare function transformToSdkRequest(body: any, model: string, auth: KiroAuthDetails, think?: boolean, budget?: number, showToast?: ToastFunction): SdkPreparedRequest;
export {};
