<script lang="ts">
	import Badge from '$components/Badge.svelte';
	import Button from '$components/Button.svelte';
	import Icon from '$components/Icon.svelte';
	import LineStats from '$components/LineStats.svelte';
	import ExecutableLabel from '$components/file/ExecutableLabel.svelte';
	import FileName from '$components/file/FileName.svelte';
	import FileStatusBadge from '$components/file/FileStatusBadge.svelte';
	import type { FileStatus } from '$components/file/types';

	interface Props {
		id?: string;
		filePath: string;
		fileStatus?: FileStatus;
		fileStatusTooltip?: string;
		draggable?: boolean;
		linesAdded?: number;
		linesRemoved?: number;
		conflicted?: boolean;
		executable?: boolean;
		transparent?: boolean;
		noPaddings?: boolean;
		oncontextmenu?: (e: MouseEvent) => void;
		oncloseclick?: () => void;
	}

	const {
		id,
		filePath,
		fileStatus,
		fileStatusTooltip,
		draggable,
		linesAdded = 0,
		linesRemoved = 0,
		conflicted,
		executable,
		transparent,
		noPaddings,
		oncontextmenu,
		oncloseclick
	}: Props = $props();
</script>

<div
	role="presentation"
	{id}
	class="file-header"
	class:draggable
	class:transparent
	class:no-paddings={noPaddings}
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
		{#if linesAdded > 0 || linesRemoved > 0}
			<LineStats {linesAdded} {linesRemoved} />
		{/if}

		{#if executable}
			<ExecutableLabel />
		{/if}

		{#if fileStatus}
			<FileStatusBadge tooltip={fileStatusTooltip} status={fileStatus} style="full-large" />
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
		width: 100%;
		padding: 12px 10px 12px 14px;
		gap: 12px;

		&.transparent {
			background-color: transparent;
		}

		&.draggable {
			cursor: grab;

			&:hover {
				& .file-header__drag-handle {
					opacity: 1;
				}
			}
		}

		&.no-paddings {
			padding: 0;
		}
	}

	.file-header__statuses {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.file-header__name {
		display: flex;
		flex: 1;
		align-items: center;
		overflow: hidden;
	}

	.file-header__drag-handle {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 10px;
		margin-right: -10px;
		margin-left: -8px;
		color: var(--clr-text-3);
		opacity: 0;
		transition:
			width var(--transition-fast),
			opacity var(--transition-fast);
	}
</style>
