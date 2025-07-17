<script lang="ts">
	import Badge from '$lib/Badge.svelte';
	import Checkbox from '$lib/Checkbox.svelte';
	import Icon from '$lib/Icon.svelte';
	import Tooltip from '$lib/Tooltip.svelte';
	import FileIcon from '$lib/file/FileIcon.svelte';
	import FileStatusBadge from '$lib/file/FileStatusBadge.svelte';
	import { splitFilePath } from '$lib/utils/filePath';
	import type { FileStatus } from '$lib/file/types';

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
	class:open
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
					<Badge style="success">Resolved</Badge>
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
		display: flex;
		align-items: center;
		height: 32px;
		padding: 6px 14px;
		overflow: hidden;
		gap: 10px;
		border-bottom: none;
		outline: none;
		background: transparent;
		text-align: left;
		user-select: none;

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
		margin-right: -8px;
		color: var(--clr-text-3);
		opacity: 0;
		transition:
			width var(--transition-fast),
			opacity var(--transition-fast);
	}

	.mark-resolved-btn {
		display: flex;
		align-items: center;
		margin: 0 2px;
		padding: 3px 6px 3px 6px;
		gap: 4px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		white-space: nowrap;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1);
		}
	}

	.info {
		display: flex;
		flex-grow: 1;
		flex-shrink: 1;
		align-items: center;
		min-width: 32px;
		overflow: hidden;
		gap: 6px;
	}

	.name {
		flex-grow: 0;
		flex-shrink: 1;
		min-width: 40px;
		color: var(--clt-text-1);
		pointer-events: none;
	}

	.path-container {
		display: flex;
		flex-grow: 1;
		flex-shrink: 0;
		flex-basis: 0px;
		justify-content: flex-start;
		min-width: 50px;
		overflow: hidden;
		text-align: left;
	}

	.path {
		display: inline-block;
		max-width: 100%;
		color: var(--clt-text-1);
		line-height: 120%;
		text-align: left;
		opacity: 0.3;
		transition: opacity var(--transition-fast);
	}

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

	.selected-draggable {
		background-color: var(--clr-theme-pop-bg-muted) !important;
	}
</style>
