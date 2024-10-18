import { buildContext } from '$lib/context';
import { get, writable } from 'svelte/store';

interface RoutesService {
	repositories(): string;
	repository(repositoryId: string): string;
	patchStack(repositoryId: string, patchStackId: string): string;
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

	patchStack(repositoryId: string, patchStackId: string): string {
		return this.externalizePath(`/repositories/${repositoryId}/patchStacks/${patchStackId}`);
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
	currentProjectId = writable<string | undefined>();

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

	patchStack(repositoryId: string, patchStackId: string): string {
		const projectId = get(this.currentProjectId);
		if (projectId) {
			return `/${projectId}/series/patchStacks/${patchStackId}`;
		}
		return this.webRoutesService.patchStack(repositoryId, patchStackId);
	}
}

export const [getRoutesService, setRoutesService] = buildContext<RoutesService>('routes-service');
