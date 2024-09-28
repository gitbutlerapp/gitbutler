export interface GitHostIssueService {
	create(title: string, body: string, labels: string[]): Promise<void>;
	listLabels(): Promise<string[]>;
}
