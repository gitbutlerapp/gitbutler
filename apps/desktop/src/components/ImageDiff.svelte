<script lang="ts">
	import { FILE_SERVICE } from '$lib/files/fileService';
	import { inject } from '@gitbutler/core/context';
	import type { TreeChange } from '$lib/hunks/change';

	type Props = {
		projectId: string;
		change: TreeChange;
		/** If provided, this is a commit diff (not a workspace diff). */
		commitId?: string;
	};

	type ImageSource =
		| { type: 'workspace'; path: string }
		| { type: 'commit'; path: string; commitId: string }
		| { type: 'blob'; path: string; blobId: string };

	type LoadStrategy = {
		before: ImageSource | null;
		after: ImageSource | null;
	};

	const { projectId, change, commitId }: Props = $props();
	const fileService = inject(FILE_SERVICE);

	let beforeImageUrl = $state<string | null>(null);
	let afterImageUrl = $state<string | null>(null);
	let loadError = $state<string | null>(null);

	// Decide image sources for before/after panels without changing logic.
	function getLoadStrategy(): LoadStrategy {
		const { status, path } = change;
		const isCommitDiff = !!commitId;

		switch (status.type) {
			case 'Addition':
				return isCommitDiff
					? { before: null, after: { type: 'commit' as const, path, commitId: commitId! } }
					: { before: null, after: { type: 'workspace' as const, path } };

			case 'Deletion':
				return isCommitDiff
					? { before: { type: 'commit' as const, path, commitId: `${commitId}^` }, after: null }
					: {
							before: { type: 'blob' as const, path, blobId: status.subject.previousState.id },
							after: null
						};

			case 'Modification':
				return isCommitDiff
					? {
							before: { type: 'commit' as const, path, commitId: `${commitId}^` },
							after: { type: 'commit' as const, path, commitId: commitId! }
						}
					: {
							before: { type: 'blob' as const, path, blobId: status.subject.previousState.id },
							after: { type: 'workspace' as const, path }
						};

			case 'Rename':
				return isCommitDiff
					? {
							before: {
								type: 'commit' as const,
								path: status.subject.previousPath,
								commitId: `${commitId}^`
							},
							after: { type: 'commit' as const, path, commitId: commitId! }
						}
					: {
							before: {
								type: 'blob' as const,
								path: status.subject.previousPath,
								blobId: status.subject.previousState.id
							},
							after: { type: 'workspace' as const, path }
						};
		}
	}

	// Load image from workspace, commit, or blob.
	async function loadImage(
		source: ImageSource | null,
		signal?: AbortSignal
	): Promise<string | null> {
		if (!source) return null;

		try {
			let fileInfo;

			if (source.type === 'workspace') {
				const { data } = await fileService.readFromWorkspace(source.path, projectId);
				fileInfo = data;
			} else if (source.type === 'commit') {
				fileInfo = await fileService.readFromCommit(source.path, projectId, source.commitId);
			} else {
				// type === 'blob'
				fileInfo = await fileService.readFromBlob(source.path, projectId, source.blobId);
			}

			if (signal?.aborted) return null;

			if (fileInfo?.content && fileInfo?.mimeType) {
				return `data:${fileInfo.mimeType};base64,${fileInfo.content}`;
			}

			return null;
		} catch (err) {
			if (signal?.aborted) return null;
			console.warn(`Failed to load image from ${source.type}: ${source.path}`, err);
			return null;
		}
	}

	// Load both images according to the strategy.
	async function loadImages(signal?: AbortSignal) {
		loadError = null;
		beforeImageUrl = null;
		afterImageUrl = null;

		try {
			const strategy = getLoadStrategy();

			const [before, after] = await Promise.all([
				loadImage(strategy.before, signal),
				loadImage(strategy.after, signal)
			]);

			if (signal?.aborted) return;

			beforeImageUrl = before;
			afterImageUrl = after;

			if (!before && !after) {
				loadError = 'Failed to load both images (before and after).';
			} else if (!before && strategy.before) {
				loadError = 'Failed to load before image.';
			} else if (!after && strategy.after) {
				loadError = 'Failed to load after image.';
			}
		} catch (err) {
			console.error('Failed to load images:', err);
			loadError = `Failed to load images: ${err instanceof Error ? err.message : String(err)}`;
		}
	}

	$effect(() => {
		const abortController = new AbortController();
		loadImages(abortController.signal);
		return () => abortController.abort();
	});
</script>

{#if loadError}
	<div class="error-message">
		<p>{loadError}</p>
	</div>
{:else}
	<div class="image-diff-container">
		{#if beforeImageUrl || afterImageUrl}
			<div class="image-comparison">
				{#if beforeImageUrl}
					<div class="image-panel before">
						<div class="image-header">
							<span class="label">Before</span>
						</div>
						<div class="image-wrapper">
							<img src={beforeImageUrl} alt="{change.path} (Before)" />
						</div>
					</div>
				{/if}

				{#if afterImageUrl}
					<div class="image-panel after">
						<div class="image-header">
							<span class="label">After</span>
						</div>
						<div class="image-wrapper">
							<img src={afterImageUrl} alt="{change.path} (After)" />
						</div>
					</div>
				{/if}
			</div>
		{:else}
			<div class="loading">Loading images...</div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.error-message {
		padding: 20px;
		color: var(--clr-scale-warn-40);
		text-align: center;
	}

	.image-diff-container {
		padding: 14px;
		border: 1px solid var(--clr-border-3);
		border-radius: var(--radius-m);
		background: var(--clr-bg-2);
	}

	.image-comparison {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
		gap: 14px;
	}

	.image-panel {
		overflow: hidden;
		border-radius: var(--radius-s);
		background: var(--clr-bg-1);

		&.before {
			border: 2px solid var(--clr-scale-err-40);
		}

		&.after {
			border: 2px solid var(--clr-scale-succ-40);
		}
	}

	.image-header {
		padding: 8px 12px;
		font-weight: 600;
		font-size: 12px;
		letter-spacing: 0.5px;
		text-transform: uppercase;

		.before & {
			background: var(--clr-scale-err-40);
			color: var(--clr-core-ntrl-100);
		}

		.after & {
			background: var(--clr-scale-succ-40);
			color: var(--clr-core-ntrl-100);
		}
	}

	.label {
		display: inline-block;
	}

	.image-wrapper {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 200px;
		padding: 14px;
		background-image:
			linear-gradient(45deg, var(--clr-bg-3) 25%, transparent 25%),
			linear-gradient(-45deg, var(--clr-bg-3) 25%, transparent 25%),
			linear-gradient(45deg, transparent 75%, var(--clr-bg-3) 75%),
			linear-gradient(-45deg, transparent 75%, var(--clr-bg-3) 75%);
		background-position:
			0 0,
			0 10px,
			10px -10px,
			-10px 0px;
		background-size: 20px 20px;
	}

	.image-wrapper img {
		display: block;
		max-width: 100%;
		max-height: 600px;
		object-fit: contain;
		border-radius: var(--radius-s);
	}

	.loading {
		padding: 40px;
		color: var(--clr-scale-ntrl-50);
		text-align: center;
	}
</style>
