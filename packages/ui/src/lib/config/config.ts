import { persisted, type Persisted } from '@square/svelte-store';

export function projectHttpsWarningBannerDismissed(projectId: string): Persisted<boolean> {
	const key = 'projectHttpsWarningBannerDismissed_';
	return persisted(false, key + projectId);
}

export function projectMergeUpstreamWarningDismissed(projectId: string): Persisted<boolean> {
	const key = 'projectMergeUpstreamWarningDismissed_';
	return persisted(false, key + projectId);
}

export function projectCommitGenerationExtraConcise(projectId: string): Persisted<boolean> {
	const key = 'projectCommitGenerationExtraConcise_';
	return persisted(false, key + projectId);
}

export function projectCommitGenerationUseEmojis(projectId: string): Persisted<boolean> {
	const key = 'projectCommitGenerationUseEmojis_';
	return persisted(false, key + projectId);
}
