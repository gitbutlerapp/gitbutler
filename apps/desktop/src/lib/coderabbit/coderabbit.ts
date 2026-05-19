import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/backendApi";

export const CODERABBIT_SERVICE = new InjectionToken<CodeRabbitService>("CodeRabbitService");

export type CodeRabbitSeverity = "critical" | "major" | "minor" | "info";
export type CodeRabbitFindingStatus = "open" | "dismissed" | "applied";
export type CodeRabbitWorkflowId = "default" | "performance" | "security" | "correctness";

export type CodeRabbitStatus = {
	cliAvailable: boolean;
	version?: string;
	authenticated: boolean;
	username?: string;
	currentOrg?: string;
	configExists: boolean;
	activeReviewId?: string;
	error?: string;
};

export type CodeRabbitReviewRequest = {
	reviewId?: string;
	reviewType?: "all" | "committed" | "uncommitted";
	base?: string;
	files?: string[];
	workflows?: CodeRabbitWorkflowId[];
};

export type CodeRabbitFinding = {
	id: string;
	reviewId: string;
	projectId: string;
	path: string;
	oldLine?: number;
	newLine?: number;
	severity: CodeRabbitSeverity;
	category?: string;
	title: string;
	body: string;
	suggestedPatch?: string;
	workflowId?: string;
	status: CodeRabbitFindingStatus;
};

export type CodeRabbitReviewResult = {
	reviewId: string;
	findings: CodeRabbitFinding[];
};

export class CodeRabbitService {
	constructor(private backendApi: BackendApi) {}

	status(projectId: string) {
		return this.backendApi.endpoints.coderabbitStatus.useQuery({ projectId });
	}

	findings(projectId: string) {
		return this.backendApi.endpoints.coderabbitFindings.useQuery({ projectId });
	}

	get review() {
		return this.backendApi.endpoints.coderabbitReview.mutate;
	}

	get login() {
		return this.backendApi.endpoints.coderabbitLogin.mutate;
	}

	get cancel() {
		return this.backendApi.endpoints.coderabbitCancel.mutate;
	}

	get updateFinding() {
		return this.backendApi.endpoints.coderabbitUpdateFinding.mutate;
	}

	get writeDefaultConfig() {
		return this.backendApi.endpoints.coderabbitWriteDefaultConfig.mutate;
	}
}
