import { CodeWhispererStreamingClient } from '@aws/codewhisperer-streaming-client';
import type { KiroAuthDetails } from './types';
export declare function createSdkClient(auth: KiroAuthDetails, region: string): CodeWhispererStreamingClient;
export declare function clearSdkClientCache(): void;
