<script lang="ts">
	import FileStatusBadge from './FileStatusBadge.svelte';
	import Checkbox from '$lib/Checkbox.svelte';
	import Icon from '$lib/Icon.svelte';
	import Tooltip from '$lib/Tooltip.svelte';
	import FileIcon from '$lib/file/FileIcon.svelte';
	import type { FileStatus } from './types';

	interface Props {
		ref?: HTMLDivElement;
		id?: string;
		fileName: string;
		filePath: string;
		fileStatus?: FileStatus;
		fileStatusStyle?: 'dot' | 'full';
		draggable?: boolean;
		selected?: boolean;
		clickable?: boolean;
		showCheckbox?: boolean;
		checked?: boolean;
		indeterminate?: boolean;
		conflicted?: boolean;
		locked?: boolean;
		lockText?: string;
		stacking?: boolean;
		oncheck?: (
			e: Event & {
				currentTarget: EventTarget & HTMLInputElement;
			}
		) => void;
		onclick?: (e: MouseEvent) => void;
		onkeydown?: (e: KeyboardEvent) => void;
		ondragstart?: (e: DragEvent) => void;
		oncontextmenu?: (e: MouseEvent) => void;
	}

	let {
		ref = $bindable(),
		id,
		fileName,
		filePath,
		fileStatus,
		fileStatusStyle = 'dot',
		draggable = false,
		selected = false,
		clickable = true,
		showCheckbox = false,
		checked = $bindable(),
		indeterminate,
		conflicted,
		locked,
		lockText,
		stacking = false,
		oncheck,
		onclick,
		onkeydown,
		ondragstart,
		oncontextmenu
	}: Props = $props();
</script>

<div
	bind:this={ref}
	{id}
	data-locked={locked}
	class="file-list-item"
	class:selected-draggable={selected}
	class:clickable
	class:draggable
	class:stacking
	aria-selected={selected}
	role="option"
	tabindex="-1"
	{onclick}
	{onkeydown}
	oncontextmenu={(e) => {
		if (oncontextmenu) {
			e.preventDefault();
			oncontextmenu(e);
		}
	}}
	ondragstart={(e) => {
		if (draggable) {
			ondragstart?.(e);
		}
	}}
>
	{#if showCheckbox}
		<Checkbox small {checked} {indeterminate} onchange={oncheck} />
	{/if}
	<div class="info">
		<FileIcon {fileName} size={14} />
		<span class="text-12 name">
			{fileName}
		</span>
		<span class="text-12 path">
			{filePath}
		</span>
	</div>

	<div class="details">
		{#if locked}
			<Tooltip text={lockText}>
				<div class="locked">
					<Icon name="locked-small" color="warning" />
				</div>
			</Tooltip>
		{/if}

		{#if conflicted}
			<div class="conflicted">
				<Icon name="warning-small" color="error" />
			</div>
		{/if}

		{#if fileStatus}
			<FileStatusBadge status={fileStatus} style={fileStatusStyle} />
		{/if}

		{#if draggable}
			<div class="draggable-handle">
				<Icon name="draggable-narrow" />
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	.file-list-item {
		flex: 1;
		display: flex;
		align-items: center;
		padding: 6px 14px;
		gap: 10px;
		height: 32px;
		overflow: hidden;
		text-align: left;
		user-select: none;
		outline: none;
		background: var(--clr-bg-1);
		border-bottom: 1px solid var(--clr-border-3);

		&:last-child {
			border-bottom: none;
		}

		&.stacking {
			background: transparent;
			border-bottom: none;
		}

		&:not(:last-child).stacking {
			border-bottom: 1px solid var(--clr-border-3);
		}
	}

	.file-list-item.clickable {
		cursor: pointer;

		&:not(.selected-draggable):hover {
			background-color: var(--clr-bg-1-muted);
		}
	}

	.draggable {
		&:hover {
			& .draggable-handle {
				opacity: 1;
			}
		}
	}

	.draggable-handle {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 4px;
		color: var(--clr-text-3);
		opacity: 0;
		margin-right: -8px;
		transition:
			width var(--transition-fast),
			opacity var(--transition-fast);
	}

	.info {
		pointer-events: none;
		display: flex;
		align-items: center;
		flex-grow: 1;
		flex-shrink: 1;
		gap: 6px;
		overflow: hidden;
	}

	.name {
		color: var(--clr-scale-ntrl-0);
		white-space: nowrap;
		flex-shrink: 0;
		text-overflow: ellipsis;
		overflow: hidden;
	}

	.path {
		color: var(--clr-scale-ntrl-0);
		line-height: 120%;
		flex-shrink: 1;
		white-space: nowrap;
		text-overflow: ellipsis;
		overflow: hidden;
		opacity: 0.3;
	}

	/* DETAILS */
	.details {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.details .locked {
		display: flex;
	}

	.details .conflicted {
		display: flex;
	}

	/* MODIFIERS */

	.selected-draggable {
		background-color: var(--clr-theme-pop-bg);
	}
</style>
