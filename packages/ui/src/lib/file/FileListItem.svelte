<script lang="ts">
	import FileStatusBadge from './FileStatusBadge.svelte';
	import Badge from '$lib/Badge.svelte';
	import Checkbox from '$lib/Checkbox.svelte';
	import Icon from '$lib/Icon.svelte';
	import Tooltip from '$lib/Tooltip.svelte';
	import FileIcon from '$lib/file/FileIcon.svelte';
	import { splitFilePath } from '$lib/utils/filePath';
	import type { FileStatus } from './types';

	interface Props {
		ref?: HTMLDivElement;
		id?: string;
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
		conflictHint?: string;
		locked?: boolean;
		lockText?: string;
		oncheck?: (
			e: Event & {
				currentTarget: EventTarget & HTMLInputElement;
			}
		) => void;
		onclick?: (e: MouseEvent) => void;
		onresolveclick?: (e: MouseEvent) => void;
		onkeydown?: (e: KeyboardEvent) => void;
		oncontextmenu?: (e: MouseEvent) => void;
	}

	let {
		ref = $bindable(),
		id,
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
		conflictHint,
		locked,
		lockText,
		oncheck,
		onclick,
		onresolveclick,
		onkeydown,
		oncontextmenu
	}: Props = $props();

	const fileInfo = $derived(splitFilePath(filePath));
</script>

<div
	bind:this={ref}
	data-locked={locked}
	data-file-id={id}
	class="file-list-item"
	class:selected-draggable={selected}
	class:clickable
	class:draggable
	aria-selected={selected}
	role="option"
	tabindex="-1"
	{onclick}
	{onkeydown}
	oncontextmenu={(e) => {
		if (oncontextmenu) {
			e.preventDefault();
			e.stopPropagation();
			oncontextmenu(e);
		}
	}}
>
	{#if showCheckbox}
		<Checkbox small {checked} {indeterminate} onchange={oncheck} />
	{/if}
	<div class="info">
		<FileIcon fileName={fileInfo.filename} />
		<span class="text-12 text-semibold name truncate">
			{fileInfo.filename}
		</span>

		<div class="path-container">
			<Tooltip text={filePath} delay={1200}>
				<span class="text-12 path truncate">
					{fileInfo.path}
				</span>
			</Tooltip>
		</div>
	</div>

	<div class="details">
		{#if locked}
			<Tooltip text={lockText}>
				<div class="locked">
					<Icon name="locked-small" color="warning" />
				</div>
			</Tooltip>
		{/if}

		{#if onresolveclick}
			{#if !conflicted}
				<Tooltip text="Conflict resolved">
					<Badge style="success" label="Resolved" />
				</Tooltip>
			{:else}
				<button
					type="button"
					class="mark-resolved-btn"
					onclick={(e) => {
						e.stopPropagation();
						onresolveclick?.(e);
					}}
				>
					<span class="text-11 text-semibold">Mark as resolved</span>
					<Icon name="tick-small" opacity={0.5} />
				</button>
			{/if}
		{/if}

		{#if conflicted}
			<Tooltip text={conflictHint}>
				<div class="conflicted">
					<Icon name="warning-small" color="error" />
				</div>
			</Tooltip>
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
		background: transparent;
		border-bottom: none;

		&:last-child {
			border-bottom: none;
		}

		&:not(:last-child) {
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

	.mark-resolved-btn {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 3px 6px 3px 6px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		margin: 0 2px;
		white-space: nowrap;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1);
		}
	}

	/* INFO */
	.info {
		display: flex;
		align-items: center;
		flex-grow: 1;
		flex-shrink: 1;
		gap: 6px;
		overflow: hidden;
	}

	.name {
		flex-shrink: 1;
		flex-grow: 0;
		min-width: 40px;
		pointer-events: none;
		color: var(--clt-text-1);
	}

	.path-container {
		display: flex;
		justify-content: flex-end;
		flex-shrink: 0;
		flex-grow: 1;
		flex-basis: 0px;
		min-width: 50px;
		direction: rtl;
		text-align: left;
		overflow: hidden;
	}

	.path {
		display: inline-block;
		color: var(--clt-text-1);
		line-height: 120%;
		opacity: 0.3;
		transition: opacity var(--transition-fast);
		direction: rtl;
		max-width: 100%;
		text-align: left;
	}

	/* DETAILS */
	.details {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.details .locked {
		display: flex;
	}

	.details .conflicted {
		display: flex;
	}

	/* MODIFIERS */
	.selected-draggable {
		background-color: var(--clr-theme-pop-bg) !important;
	}
</style>
