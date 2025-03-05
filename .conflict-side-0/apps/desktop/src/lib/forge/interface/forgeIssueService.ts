export interface ForgeIssueService {
	create(title: string, body: string, labels: string[]): Promise<void>;
	listLabels(): Promise<string[]>;
}
