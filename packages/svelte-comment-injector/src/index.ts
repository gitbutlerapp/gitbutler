import path from 'path';

/**
 * Svelte preprocessor that injects HTML comments at the start and end of each component — in dev only.
 */
export default function svelteDevtoolsComment() {
	const isDev = process.env.NODE_ENV === 'development';

	return {
		markup({ content, filename }: { content: string; filename?: string }) {
			if (!isDev) return { code: content };

			const file = filename ? path.basename(filename) : 'Unknown.svelte';
			const start = `{@html '<!-- Begin ${file} -->'}`;
			const end = `{@html '<!-- End ${file} -->'}`;

			// Inject start after the opening script/style blocks, and end at the bottom
			const injected = `${start}\n${content}\n${end}`;
			return { code: injected };
		}
	};
}
