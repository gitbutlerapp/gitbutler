export interface OwnerParameters {
	ownerSlug: string;
}

export interface ProjectParameters extends OwnerParameters {
	projectSlug: string;
}

export interface ProjectReviewParameters extends ProjectParameters {
	branchId: string;
}

export interface ProjectReviewCommitParameters extends ProjectReviewParameters {
	changeId: string;
}

export class WebRoutesService {
	constructor(private readonly baseUrl: string) {}

	private toUrl(path: string) {
		return `${this.baseUrl}${path}`;
	}

	projectsPath() {
		return `/repositories`;
	}
	projectsUrl() {
		return this.toUrl(this.projectsPath());
	}

	ownerPath(parameters: OwnerParameters) {
		return `/${parameters.ownerSlug}`;
	}
	ownerUrl(parameters: OwnerParameters) {
		return this.toUrl(this.ownerPath(parameters));
	}

	projectPath(parameters: ProjectParameters) {
		return `/${parameters.ownerSlug}/${parameters.projectSlug}`;
	}
	projectUrl(parameters: ProjectParameters) {
		return this.toUrl(this.projectPath(parameters));
	}

	projectReviewPath(parameters: ProjectParameters) {
		return `/${parameters.ownerSlug}/${parameters.projectSlug}/reviews`;
	}
	projectReviewUrl(parameters: ProjectParameters) {
		return this.toUrl(this.projectReviewPath(parameters));
	}

	projectReviewBranchPath(parameters: ProjectReviewParameters) {
		return `/${parameters.ownerSlug}/${parameters.projectSlug}/reviews/${parameters.branchId}`;
	}
	projectReviewBranchUrl(parameters: ProjectReviewParameters) {
		return this.toUrl(this.projectReviewBranchPath(parameters));
	}

	projectReviewBranchCommitPath(parameters: ProjectReviewCommitParameters) {
		return `/${parameters.ownerSlug}/${parameters.projectSlug}/reviews/${parameters.branchId}/commit/${parameters.changeId}`;
	}
	projectReviewBranchCommitUrl(parameters: ProjectReviewCommitParameters) {
		return this.toUrl(this.projectReviewBranchCommitPath(parameters));
	}
}
