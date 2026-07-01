import path from "path";

interface svelteInjectOptions {
	enabled?: boolean;
	showEndComment?: boolean;
	showFullPath?: boolean;
}

/**
 * Svelte preprocessor that injects HTML comments at the start and end of each component — in dev only.
 */
export default function svelteInjectComment(options: svelteInjectOptions = {}) {
	const {
		enabled = process.env.NODE_ENV === "development",
		showEndComment = true,
		showFullPath = false,
	} = options;

	return {
		markup({ content, filename }: { content: string; filename?: string }): { code: string } {
			if (!enabled || !filename) return { code: content };

			const filePath = showFullPath
				? filename.replace(process.cwd() + "/", "")
				: path.basename(filename);

			const startComment = `{@html '<!-- Begin ${filePath} -->'}`;
			const endComment = `{@html '<!-- End ${filePath} -->'}`;

			// Inject start after the opening script/style blocks, and end at the bottom
			const injected = showEndComment
				? `${startComment}\n${content}\n${endComment}`
				: `${startComment}\n${content}`;
			return { code: injected };
		},
	};
}
