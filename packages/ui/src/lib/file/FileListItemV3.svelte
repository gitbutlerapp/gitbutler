<script lang="ts">
	import Badge from '$lib/Badge.svelte';
	import Button from '$lib/Button.svelte';
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
		size?: 'small' | 'large';
		draggable?: boolean;
		selected?: boolean;
		focused?: boolean;
		clickable?: boolean;
		showCheckbox?: boolean;
		listMode: 'list' | 'tree';
		checked?: boolean;
		indeterminate?: boolean;
		conflicted?: boolean;
		conflictHint?: string;
		locked?: boolean;
		lockText?: string;
		listActive?: boolean;
		oncheck?: (
			e: Event & {
				currentTarget: EventTarget & HTMLInputElement;
			}
		) => void;
		onclick?: (e: MouseEvent) => void;
		ondblclick?: (e: MouseEvent) => void;
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
		size = 'small',
		draggable = false,
		selected = false,
		focused = false,
		clickable = true,
		showCheckbox = false,
		checked = $bindable(),
		indeterminate,
		conflicted,
		conflictHint,
		locked,
		lockText,
		listActive,
		listMode,
		oncheck,
		onclick,
		ondblclick,
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
	class="file-list-item size-{size}"
	class:selected
	class:list-active={listActive}
	class:clickable
	class:draggable
	class:focused
	class:list-mode={listMode === 'list'}
	aria-selected={selected}
	role="option"
	tabindex="-1"
	{onclick}
	{ondblclick}
	{onkeydown}
	oncontextmenu={(e) => {
		if (oncontextmenu) {
			e.preventDefault();
			e.stopPropagation();
			oncontextmenu(e);
		}
	}}
>
	{#if draggable && !showCheckbox}
		<div class="draggable-handle">
			<Icon name="draggable-narrow" />
		</div>
	{/if}
	{#if showCheckbox}
		<Checkbox small {checked} {indeterminate} onchange={oncheck} />
	{/if}
	<div class="info">
		<FileIcon fileName={fileInfo.filename} />
		<span class="text-12 text-semibold name truncate">
			{fileInfo.filename}
		</span>

		{#if listMode === 'list' && fileInfo.path}
			<div class="path-container">
				<Tooltip text={filePath} delay={1200}>
					<span class="text-12 path truncate">
						{fileInfo.path}
					</span>
				</Tooltip>
			</div>
		{/if}
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
				<Button
					type="button"
					kind="outline"
					class="mark-resolved-btn"
					size="tag"
					onclick={(e) => {
						e.stopPropagation();
						onresolveclick?.(e);
					}}
					icon="tick-small"
				>
					Mark as resolved
				</Button>
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
	</div>
</div>

<style lang="postcss">
	.file-list-item {
		display: flex;
		align-items: center;
		padding: 6px 12px 6px 14px;
		gap: 10px;
		height: 32px;
		overflow: hidden;
		text-align: left;
		user-select: none;
		outline: none;
		background: transparent;

		& :global(.mark-resolved-btn) {
			margin: 0 4px;
		}

		&.list-mode {
			border-bottom: 1px solid var(--clr-border-3);
		}

		&.size-large {
			padding: 14px;
			height: unset;
			&.list-mode {
				border-bottom: 1px solid var(--clr-border-2);
			}
		}
	}

	.file-list-item.clickable {
		cursor: pointer;

		&:not(.selected):hover {
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
		color: var(--clr-text-3);
		opacity: 0;
		height: 24px;
		margin-left: -14px;
		margin-right: -12px;
		transition: opacity var(--transition-fast);
	}

	.info {
		display: flex;
		align-items: center;
		flex-shrink: 1;
		min-width: 32px;
		gap: 6px;
		width: 100%;
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
		justify-content: flex-start;
		flex-shrink: 0;
		flex-grow: 1;
		flex-basis: 0px;
		text-align: left;
		min-width: 16px;
		overflow: hidden;
	}

	.path {
		display: inline-block;
		color: var(--clt-text-1);
		line-height: 120%;
		opacity: 0.3;
		max-width: 100%;
		text-align: left;
	}

	.details {
		flex-grow: 1;
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

	.selected {
		background-color: var(--clr-selected-not-in-focus-bg);
	}

	.list-active.selected {
		background-color: var(--clr-selected-in-focus-bg);
	}
</style>
