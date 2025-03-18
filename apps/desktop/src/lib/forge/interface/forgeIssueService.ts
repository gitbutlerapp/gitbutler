import type { CreateIssueResult } from '../github/types';

export interface ForgeIssueService {
	create(title: string, body: string, labels: string[]): Promise<CreateIssueResult>;
}
