export { default as RenderedSearchResult } from './RenderedSearchResult.svelte';

import type { SearchResult } from '$lib/search';

export type ProcessedSearchResultLine = {
	hidden: boolean;
	content: string;
	operation: string;
	lineNumber: number | undefined;
	hasKeyword: boolean;
};

export type ProcessedSearchRestultHunk = {
	lines: ProcessedSearchResultLine[];
};

export type ProcessedSearchResult = {
	searchResult: SearchResult;
	hunks: ProcessedSearchRestultHunk[];
	timestamp: Date;
};
