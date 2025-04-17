import { getContext, setContext } from 'svelte';

export type ExternalLinkService = {
	open(href: string): void;
};

const key = Symbol('ExternalLinkService');

export function getExternalLinkService() {
	const externalLinkService = getContext<ExternalLinkService | undefined>(key);
	if (!externalLinkService) throw new Error('External link service not provided');
	return externalLinkService;
}

export function setExternalLinkService(externalLinkService: ExternalLinkService) {
	setContext(key, externalLinkService);
}
