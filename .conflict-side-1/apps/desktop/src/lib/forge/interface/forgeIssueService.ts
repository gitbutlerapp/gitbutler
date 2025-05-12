import type { CreateIssueResult } from '$lib/forge/github/types';

export interface ForgeIssueService {
	create(title: string, body: string, labels: string[]): Promise<CreateIssueResult>;
}
