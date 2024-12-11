import { persistWithExpiration, type Persisted } from '@gitbutler/shared/persisted';
import type { DetailedCommit } from '$lib/vbranches/types';

export function persistedPRBody(projectId: string, seriesName: string): Persisted<string> {
	return persistWithExpiration('', 'seriesCurrentPRBody_' + projectId + '_' + seriesName, 5);
}

export function persistedPRTitle(projectId: string, seriesName: string): Persisted<string> {
	return persistWithExpiration('', 'seriesCurrentPRTitle_' + projectId + '_' + seriesName, 5);
}

export interface PRContent {
	title: string;
	body: string;
	templateBody: string | undefined;
}

export interface PRContentParams {
	isDisplay: boolean;
	projectId: string;
	seriesName: string;
	seriesDescription: string | undefined;
	existingTitle: string | undefined;
	existingBody: string | undefined;
	commits: DetailedCommit[];
}

export default function getPRContent(params: PRContentParams): PRContent {
	let templateBody = $state<string | undefined>(undefined);
	const defaultTitle: string = $derived.by(() => {
		if (params.isDisplay) return params.existingTitle ?? '';

		// In case of a single commit, use the commit summary for the title
		if (params.commits.length === 1) {
			const commit = params.commits[0];
			return commit?.descriptionTitle ?? '';
		}

		return params.seriesName;
	});

	const defaultBody: string = $derived.by(() => {
		if (params.isDisplay) return params.existingBody ?? '';
		if (params.seriesDescription) return params.seriesDescription;
		if (templateBody) return templateBody;

		// In case of a single commit, use the commit description for the body
		if (params.commits.length === 1) {
			const commit = params.commits[0];
			return commit?.descriptionBody ?? '';
		}

		return '';
	});

	const persistedBody = persistedPRBody(params.projectId, params.seriesName);
	const persistedTitle = persistedPRTitle(params.projectId, params.seriesName);

	let inputBody = $state<string>();
	let inputTitle = $state<string>();

	let actualBody = $state<string>('');
	let actualTitle = $state<string>('');

	$effect(() => {
		actualBody = defaultBody;
	});

	$effect(() => {
		actualBody = inputBody || '';
	});

	persistedBody.subscribe((value) => {
		inputBody = value;
	});

	$effect(() => {
		actualTitle = defaultTitle;
	});

	$effect(() => {
		actualTitle = inputTitle || '';
	});

	persistedTitle.subscribe((value) => {
		inputTitle = value;
	});

	return {
		get title() {
			return actualTitle;
		},

		set title(value: string) {
			inputTitle = value;
		},

		get body() {
			return actualBody;
		},

		set body(value: string) {
			inputBody = value;
		},

		get templateBody() {
			return templateBody;
		},

		set templateBody(value: string | undefined) {
			templateBody = value;
		}
	};
}
