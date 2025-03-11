import type { Patch } from './hunk';

/**
 * A patch in unified diff format to show how a resource changed or now looks
 * like (in case it was newly added), or how it previously looked like in case
 * of a deletion.
 */
export type UnifiedDiff =
	| { readonly type: 'Binary' } // A binary file that can't be diffed.
	| { readonly type: 'TooLarge'; readonly subject: TooLarge }
	| { readonly type: 'Patch'; readonly subject: Patch };

/** The file was too large and couldn't be diffed. */
export type TooLarge = {
	/** The size of the file on disk that made it too large. */
	readonly sizeInBytes: number;
};
