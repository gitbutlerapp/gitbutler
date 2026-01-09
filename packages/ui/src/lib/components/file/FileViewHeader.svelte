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
		solid?: boolean;
		noPaddings?: boolean;
		pathFirst?: boolean;
		sticky?: boolean;
		topBorder?: boolean;
		bottomBorder?: boolean;
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
		solid,
		noPaddings,
		pathFirst = true,
		sticky,
		topBorder,
		bottomBorder,
		oncontextmenu,
		oncloseclick
	}: Props = $props();
</script>

<div
	role="presentation"
	{id}
	class="file-header"
	class:draggable
	class:solid
	class:sticky
	class:top-border={topBorder}
	class:bottom-border={bottomBorder}
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
		<FileName {filePath} {pathFirst} textSize="13" />
	</div>

	<div class="file-header__statuses">
		{#if linesAdded > 0 || linesRemoved > 0}
			<LineStats {linesAdded} {linesRemoved} />
		{/if}

		{#if executable}
			<ExecutableLabel />
		{/if}

		{#if fileStatus}
			<FileStatusBadge tooltip={fileStatusTooltip} status={fileStatus} style="full" />
		{/if}

		{#if conflicted}
			<Badge size="icon" style="danger">Has conflicts</Badge>
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
		z-index: var(--z-lifted);
		align-items: center;
		width: 100%;
		padding: 12px 10px 12px 14px;
		gap: 12px;

		&.solid {
			background-color: var(--clr-bg-1);
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

		&.top-border {
			border-top: solid 1px var(--clr-border-2);
		}

		&.bottom-border {
			border-bottom: solid 1px var(--clr-border-2);
		}

		&.sticky {
			position: sticky;
			top: 0;
		}
	}

	.file-header__statuses {
		display: flex;
		align-items: center;
		gap: 6px;
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
