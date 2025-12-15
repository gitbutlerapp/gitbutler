<script lang="ts">
	import Badge from '$components/Badge.svelte';
	import Button from '$components/Button.svelte';
	import Checkbox from '$components/Checkbox.svelte';
	import Icon from '$components/Icon.svelte';
	import Tooltip from '$components/Tooltip.svelte';
	import ExecutableLabel from '$components/file/ExecutableLabel.svelte';
	import FileIndent from '$components/file/FileIndent.svelte';
	import FileName from '$components/file/FileName.svelte';
	import FileStatusBadge from '$components/file/FileStatusBadge.svelte';
	import { focusable } from '$lib/focus/focusable';
	import type { FileStatus } from '$components/file/types';
	import type { FocusableOptions } from '$lib/focus/focusManager';

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
		listMode?: 'list' | 'tree';
		depth?: number;
		checked?: boolean;
		indeterminate?: boolean;
		conflicted?: boolean;
		conflictHint?: string;
		locked?: boolean;
		lockText?: string;
		active?: boolean;
		hideBorder?: boolean;
		executable?: boolean;
		actionOpts?: FocusableOptions;
		oncheckclick?: (e: MouseEvent) => void;
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
		onlockhover?: () => void;
		onlockunhover?: () => void;
	}

	let {
		ref = $bindable(),
		id,
		filePath,
		fileStatus,
		fileStatusTooltip,
		hideBorder,
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
		active,
		listMode = 'list',
		depth,
		executable,
		actionOpts,
		oncheck,
		oncheckclick,
		onclick,
		ondblclick,
		onresolveclick,
		onkeydown,
		oncontextmenu,
		onlockhover,
		onlockunhover
	}: Props = $props();

	const showIndent = $derived(depth && depth > 0);
</script>

<div
	bind:this={ref}
	data-locked={locked}
	data-file-id={id}
	class="file-list-item"
	class:selected
	class:active
	class:clickable
	class:focused
	class:draggable
	class:conflicted
	class:hide-border={hideBorder}
	class:list-mode={listMode === 'list'}
	aria-selected={selected}
	role="option"
	tabindex="0"
	{onclick}
	{ondblclick}
	{onkeydown}
	use:focusable={actionOpts}
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
				<Checkbox small {checked} {indeterminate} onchange={oncheck} onclick={oncheckclick} />
			{/if}
		</div>
	{/if}

	<FileName {filePath} hideFilePath={listMode === 'tree'} />

	<div class="file-list-item__details">
		{#if executable}
			<ExecutableLabel />
		{/if}

		{#if conflicted}
			<Tooltip text={conflictHint}>
				<div class="conflicted-icon">
					<Icon name="warning-small" color="danger" />
				</div>
			</Tooltip>
		{:else if fileStatus}
			<FileStatusBadge tooltip={fileStatusTooltip} status={fileStatus} style={fileStatusStyle} />
		{/if}

		{#if locked}
			<Tooltip text={lockText}>
				<div
					class="locked"
					role="img"
					aria-label="File is locked due to dependencies"
					onmouseenter={() => onlockhover?.()}
					onmouseleave={() => onlockunhover?.()}
				>
					<Icon name="locked" />
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
					class="mark-resolved-btn"
					size="tag"
					onclick={(e) => {
						e.stopPropagation();
						onresolveclick?.(e);
					}}
					icon="tick-small"
				>
					Mark resolved
				</Button>
			{/if}
		{/if}
	</div>
</div>

<style lang="postcss">
	.file-list-item {
		display: flex;
		position: relative;
		align-items: center;
		height: 32px;
		padding: 0 10px 0 14px;
		overflow: hidden;
		gap: 8px;
		outline: none;
		background-color: var(--clr-bg-1);
		text-align: left;
		user-select: none;

		& :global(.mark-resolved-btn) {
			margin: 0 4px;
		}

		&.clickable {
			cursor: pointer;

			&:not(.selected):hover {
				background-color: var(--clr-bg-1-muted);
			}

			&.conflicted:not(.selected):hover {
				background-color: var(--clr-theme-danger-bg-muted);
			}
		}

		&.conflicted {
			background-color: var(--clr-theme-danger-bg);
		}

		&.selected {
			background-color: var(--clr-selected-not-in-focus-bg);
		}

		&.active.selected {
			background-color: var(--clr-selected-in-focus-bg);
		}

		&.list-mode:not(.hide-border) {
			border-bottom: 1px solid var(--clr-border-3);
		}

		.draggable-handle {
			display: flex;
			position: absolute;
			left: 0;
			align-items: center;
			justify-content: center;
			height: 24px;
			color: var(--clr-text-3);
			opacity: 0;
			transition: opacity var(--transition-fast);
		}

		&.draggable {
			&:hover {
				& .draggable-handle {
					opacity: 1;
				}
			}
		}

		.conflicted-icon {
			display: flex;
			margin-right: -2px;
		}
	}

	.file-list-item__indicators {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.file-list-item__details {
		display: flex;
		flex-grow: 1;
		align-items: center;
		gap: 4px;

		& .locked {
			display: flex;
			color: var(--clr-change-icon-modification);
		}
	}
</style>
