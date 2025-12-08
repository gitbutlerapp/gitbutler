<script lang="ts">
	import emptyFileSvg from '$lib/assets/empty-state/empty-file.svg?raw';
	import { FILE_SERVICE } from '$lib/files/fileService';
	import { inject } from '@gitbutler/core/context';
	import { ImageDiff, EmptyStatePlaceholder } from '@gitbutler/ui';
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
	let isLoading = $state<boolean>(true);

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
		isLoading = true;
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
		} finally {
			isLoading = false;
		}
	}

	$effect(() => {
		const abortController = new AbortController();
		loadImages(abortController.signal);
		return () => abortController.abort();
	});
</script>

{#if loadError}
	<div class="imagediff-placeholder">
		<EmptyStatePlaceholder image={emptyFileSvg} gap={12} topBottomPadding={34}>
			{#snippet caption()}
				Can't preview this file type
			{/snippet}
		</EmptyStatePlaceholder>
	</div>
{:else}
	<ImageDiff {beforeImageUrl} {afterImageUrl} fileName={change.path} {isLoading} />
{/if}

<style lang="scss">
	.imagediff-placeholder {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 200px;
		border: 1px solid var(--clr-border-3);
		border-radius: var(--radius-m);
	}
</style>
