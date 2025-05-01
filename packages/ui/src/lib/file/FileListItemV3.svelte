<script lang="ts">
	import Badge from '$lib/Badge.svelte';
	import Button from '$lib/Button.svelte';
	import Checkbox from '$lib/Checkbox.svelte';
	import Icon from '$lib/Icon.svelte';
	import Tooltip from '$lib/Tooltip.svelte';
	import FileIndent from '$lib/file/FileIndent.svelte';
	import FileName from '$lib/file/FileName.svelte';
	import FileStatusBadge from '$lib/file/FileStatusBadge.svelte';
	import type { FileStatus } from '$lib/file/types';

	interface Props {
		ref?: HTMLDivElement;
		id?: string;
		filePath: string;
		fileStatus?: FileStatus;
		fileStatusTooltip?: string;
		fileStatusStyle?: 'dot' | 'full';
		draggable?: boolean;
		selected?: boolean;
		focused?: boolean;
		clickable?: boolean;
		showCheckbox?: boolean;
		listMode: 'list' | 'tree';
		depth?: number;
		checked?: boolean;
		indeterminate?: boolean;
		conflicted?: boolean;
		conflictHint?: string;
		locked?: boolean;
		lockText?: string;
		listActive?: boolean;
		isLast?: boolean;
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
		fileStatusTooltip,
		isLast,
		fileStatusStyle = 'dot',
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
		depth,
		oncheck,
		onclick,
		ondblclick,
		onresolveclick,
		onkeydown,
		oncontextmenu
	}: Props = $props();

	const showIndent = $derived(depth && depth > 0);
</script>

<div
	bind:this={ref}
	data-locked={locked}
	data-file-id={id}
	class="file-list-item"
	class:selected
	class:list-active={listActive}
	class:clickable
	class:focused
	class:draggable
	class:last={isLast}
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

	{#if showIndent || showCheckbox}
		<div class="file-list-item__indicators">
			{#if showIndent}
				<FileIndent {depth} />
			{/if}

			{#if showCheckbox}
				<Checkbox small {checked} {indeterminate} onchange={oncheck} />
			{/if}
		</div>
	{/if}

	<FileName {filePath} hideFilePath={listMode === 'tree'} />

	<div class="file-list-item__details">
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
			<FileStatusBadge tooltip={fileStatusTooltip} status={fileStatus} style={fileStatusStyle} />
		{/if}
	</div>
</div>

<style lang="postcss">
	.file-list-item {
		position: relative;
		display: flex;
		align-items: center;
		padding: 0 8px 0 14px;

		gap: 8px;
		height: 32px;
		overflow: hidden;
		text-align: left;
		user-select: none;
		outline: none;
		background: transparent;

		& :global(.mark-resolved-btn) {
			margin: 0 4px;
		}

		&.clickable {
			cursor: pointer;

			&:not(.selected):hover {
				background-color: var(--clr-bg-1-muted);
			}
		}

		&.selected {
			background-color: var(--clr-selected-not-in-focus-bg);
		}

		&.list-active.selected {
			background-color: var(--clr-selected-in-focus-bg);
		}

		&.list-mode:not(.last) {
			border-bottom: 1px solid var(--clr-border-3);
		}

		&.draggable {
			&:hover {
				& .draggable-handle {
					opacity: 1;
				}
			}
		}
	}

	.file-list-item__indicators {
		display: flex;
		align-items: center;
		gap: 6px;
		height: 100%;
	}

	.draggable-handle {
		position: absolute;
		left: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--clr-text-3);
		opacity: 0;
		height: 24px;
		transition: opacity var(--transition-fast);
	}

	.file-list-item__details {
		flex-grow: 1;
		display: flex;
		align-items: center;
		gap: 6px;

		& .locked {
			display: flex;
		}

		& .conflicted {
			display: flex;
		}
	}
</style>
