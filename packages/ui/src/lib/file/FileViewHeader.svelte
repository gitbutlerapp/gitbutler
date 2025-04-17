<script lang="ts">
	import Badge from '$lib/Badge.svelte';
	import Button from '$lib/Button.svelte';
	import Icon from '$lib/Icon.svelte';
	import FileName from '$lib/file/FileName.svelte';
	import FileStatusBadge from '$lib/file/FileStatusBadge.svelte';
	import LineChangeStats from '$lib/file/LineChangeStats.svelte';
	import type { FileStatus } from '$lib/file/types';

	interface Props {
		id?: string;
		filePath: string;
		fileStatus?: FileStatus;
		fileStatusTooltip?: string;
		draggable?: boolean;
		linesAdded?: number;
		linesRemoved?: number;
		conflicted?: boolean;
		oncontextmenu?: (e: MouseEvent) => void;
		oncloseclick?: () => void;
	}

	const {
		id,
		filePath,
		fileStatus,
		fileStatusTooltip,
		draggable = true,
		linesAdded = 0,
		linesRemoved = 0,
		conflicted = false,
		oncontextmenu,
		oncloseclick
	}: Props = $props();
</script>

<div
	role="presentation"
	{id}
	class="file-header"
	class:draggable
	{draggable}
	oncontextmenu={(e) => {
		if (oncontextmenu) {
			e.preventDefault();
			e.stopPropagation();
			oncontextmenu(e);
		}
	}}
>
	{#if draggable}
		<div class="file-header__drag-handle">
			<Icon name="draggable-narrow" />
		</div>
	{/if}

	<div class="file-header__name">
		<FileName {filePath} textSize="13" />
	</div>

	<div class="file-header__statuses">
		<LineChangeStats added={linesAdded} removed={linesRemoved} />

		{#if fileStatus}
			<FileStatusBadge tooltip={fileStatusTooltip} status={fileStatus} style="full" />
		{/if}

		{#if conflicted}
			<Badge size="icon" style="error">Has conflicts</Badge>
		{/if}
	</div>

	{#if oncloseclick}
		<Button
			class="file-header__close-btn"
			kind="ghost"
			size="tag"
			icon="cross"
			onclick={oncloseclick}
		/>
	{/if}
</div>

<style lang="postcss">
	.file-header {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 10px 12px 14px;
		width: 100%;
		background-color: var(--clr-bg-1);

		&.draggable {
			cursor: grab;

			&:hover {
				& .file-header__drag-handle {
					opacity: 1;
				}
			}
		}
	}

	.file-header__statuses {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.file-header__name {
		display: flex;
		align-items: center;
		flex: 1;
		overflow: hidden;
	}

	.file-header__drag-handle {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 10px;
		margin-left: -8px;
		margin-right: -10px;
		opacity: 0;
		color: var(--clr-text-3);
		transition:
			width var(--transition-fast),
			opacity var(--transition-fast);
	}
</style>
