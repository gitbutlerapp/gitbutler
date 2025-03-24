<script lang="ts">
	import Badge from '$lib/Badge.svelte';
	import Icon from '$lib/Icon.svelte';
	import FileName from '$lib/file/FileName.svelte';
	import FileStats from '$lib/file/FileStats.svelte';
	import { intersectionObserver } from '$lib/utils/intersectionObserver';
	import type { FileStatus } from '$lib/file/types';

	interface Props {
		id?: string;
		filePath: string;
		fileStatus?: FileStatus;
		draggable?: boolean;
		linesAdded?: number;
		linesRemoved?: number;
		conflicted?: boolean;
		hasBorder?: boolean;
		isSticky?: boolean;
		oncontextmenu?: (e: MouseEvent) => void;
	}

	const {
		id,
		filePath,
		fileStatus,
		draggable = true,
		linesAdded = 0,
		linesRemoved = 0,
		conflicted = false,
		isSticky = true,
		oncontextmenu
	}: Props = $props();

	let isIntersecting = $state(false);
</script>

<div
	role="presentation"
	{id}
	class="file-header"
	class:sticky={isSticky}
	class:intersected={isIntersecting}
	class:draggable
	{draggable}
	oncontextmenu={(e) => {
		if (oncontextmenu) {
			e.preventDefault();
			e.stopPropagation();
			oncontextmenu(e);
		}
	}}
	use:intersectionObserver={{
		callback: (entry) => {
			if (entry?.isIntersecting) {
				isIntersecting = false;
			} else {
				isIntersecting = true;
			}
		},
		options: {
			root: null,
			rootMargin: '-1px',
			threshold: 1
		}
	}}
>
	<div class="file-header__name">
		<div class="file-header__drag-handle">
			<Icon name="draggable-narrow" />
		</div>

		<FileName {filePath} textSize="13" />
	</div>

	<div class="file-header__statuses">
		<FileStats status={fileStatus} added={linesAdded} removed={linesRemoved} />
		{#if conflicted}
			<Badge size="icon" style="error">Has conflicts</Badge>
		{/if}
	</div>
</div>

<style lang="postcss">
	.file-header {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 14px;
		width: 100%;
		background-color: var(--clr-bg-1);

		&.intersected {
			border-bottom: 1px solid var(--clr-border-2);
		}

		&.sticky {
			top: -1px;
			position: sticky;
			z-index: var(--z-ground);
		}

		&.draggable {
			cursor: grab;

			&:hover {
				& .file-header__drag-handle {
					/* width: 24px; */
					opacity: 1;
				}
			}
		}
	}

	.file-header__statuses {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.file-header__name {
		display: flex;
		align-items: center;
		flex: 1;
	}

	.file-header__drag-handle {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 10px;
		margin-left: -8px;
		opacity: 0;
		color: var(--clr-text-3);
		transition:
			width var(--transition-fast),
			opacity var(--transition-fast);
	}
</style>
