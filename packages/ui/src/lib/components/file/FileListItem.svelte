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
		pathFirst?: boolean;
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
		executable?: boolean;
		isLast?: boolean;
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
		fileStatusStyle = 'dot',
		pathFirst = true,
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
		isLast = false,
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
	class:list-mode={listMode === 'list'}
	class:is-last={isLast}
	style:--file-list-item-selected-fg={selected && active
		? 'var(--clr-theme-pop-on-element)'
		: undefined}
	style:--file-list-item-selected-bg={selected && active
		? 'var(--clr-theme-pop-element)'
		: undefined}
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
				<Checkbox
					small
					{checked}
					{indeterminate}
					onchange={oncheck}
					onclick={oncheckclick}
					invertColors={selected && active}
				/>
			{/if}
		</div>
	{/if}

	<FileName
		{filePath}
		hideFilePath={listMode === 'tree'}
		{pathFirst}
		color="var(--file-list-item-selected-fg)"
	/>

	<div class="file-list-item__details">
		{#if executable}
			<ExecutableLabel />
		{/if}

		{#if conflicted}
			<Tooltip text={conflictHint}>
				<div class="conflicted-icon">
					<Icon name="warning-small" />
				</div>
			</Tooltip>
		{:else if fileStatus}
			<FileStatusBadge
				tooltip={fileStatusTooltip}
				status={fileStatus}
				style={fileStatusStyle}
				color="var(--file-list-item-selected-fg)"
			/>
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
					<Badge style="safe">Resolved</Badge>
				</Tooltip>
			{:else}
				<Button
					type="button"
					class="m-l-4 m-r-4"
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
		height: 30px;
		padding: 0 8px 0 14px;
		gap: 8px;

		background-color: var(--clr-bg-1);
		text-align: left;
		user-select: none;

		&:focus-visible {
			outline: none;
		}

		&.list-mode {
			border-bottom: 1px solid var(--clr-border-3);

			&.is-last {
				border-bottom: none;
			}
		}

		&.clickable {
			cursor: pointer;

			&:hover {
				& .draggable-handle {
					display: flex;
				}
			}

			&:not(.selected):hover {
				background-color: var(--hover-bg-1);
			}

			&.conflicted:not(.selected):hover {
				background-color: var(--hover-danger-bg);
			}
		}

		&.conflicted {
			background-color: var(--clr-theme-danger-bg);

			&.selected {
				background-color: var(--clr-theme-danger-element);
			}
		}

		&.selected {
			background-color: var(--clr-bg-2);
		}

		&.active.selected {
			border-bottom: 1px solid color-mix(in srgb, var(--file-list-item-selected-bg) 70%, white);
			background-color: var(--file-list-item-selected-bg);
		}

		.draggable-handle {
			display: none;
			position: absolute;
			top: 50%;
			left: 4px;
			align-items: center;
			justify-content: center;
			width: 6px;
			transform: translateY(-50%);
			color: var(--file-list-item-selected-fg, var(--clr-text-2));
			opacity: 0.6;
		}

		.conflicted-icon {
			display: flex;
			margin-right: -2px;
			color: var(--file-list-item-selected-fg, var(--clr-theme-danger-element));
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
			color: var(--file-list-item-selected-fg, var(--clr-change-icon-modification));
		}
	}
</style>
