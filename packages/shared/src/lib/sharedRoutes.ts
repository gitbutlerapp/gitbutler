import { buildContext } from '$lib/context';
import { get, writable, type Writable } from 'svelte/store';

export interface RoutesService {
	repositories(): string;
	repository(repositoryId: string): string;
	cloudBranch(repositoryId: string, cloudBranchId: string): string;
	patch(repositoryId: string, cloudBranchId: string, changeId: string): string;
}

export class WebRoutesService implements RoutesService {
	constructor(
		private readonly externalizePaths: boolean = false,
		private readonly webBaseUrl?: string
	) {}

	repositories() {
		return this.externalizePath('/repositories');
	}

	repository(repositoryId: string): string {
		return this.externalizePath(`/repositories/${repositoryId}`);
	}

	cloudBranch(repositoryId: string, cloudBranchId: string): string {
		return this.externalizePath(`/repositories/${repositoryId}/branches/${cloudBranchId}`);
	}

	patch(repositoryId: string, cloudBranchId: string, changeId: string): string {
		return this.externalizePath(
			`/repositories/${repositoryId}/branches/${cloudBranchId}/patches/${changeId}`
		);
	}

	private externalizePath(path: string) {
		if (this.externalizePaths) {
			return new URL(path, this.webBaseUrl).href;
		} else {
			return path;
		}
	}
}

export class DesktopRoutesService implements RoutesService {
	currentProjectId: Writable<string | undefined> = writable<string | undefined>();

	constructor(private readonly webRoutesService: WebRoutesService) {}

	repositories() {
		return this.webRoutesService.repositories();
	}

	repository(repositoryId: string): string {
		const projectId = get(this.currentProjectId);
		if (projectId) {
			return `/${projectId}/series`;
		}
		return this.webRoutesService.repository(repositoryId);
	}

	cloudBranch(repositoryId: string, cloudBranchId: string): string {
		const projectId = get(this.currentProjectId);
		if (projectId) {
			return `/${projectId}/series/branches/${cloudBranchId}`;
		}
		return this.webRoutesService.cloudBranch(repositoryId, cloudBranchId);
	}

	patch(repositoryId: string, cloudBranchId: string, changeId: string): string {
		const projectId = get(this.currentProjectId);
		if (projectId) {
			return `/${projectId}/series/branches/${cloudBranchId}/patches/${changeId}`;
		}
		return this.webRoutesService.cloudBranch(repositoryId, cloudBranchId);
	}
}

export const [getRoutesService, setRoutesService] = buildContext<RoutesService>('routes-service');
