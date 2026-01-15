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
	<div class="file-list-item__background"></div>

	<div class="file-list-item__content">
		{#if draggable && !showCheckbox}
			<!-- <div class="draggable-handle">
			<Icon name="draggable-narrow" />
		</div> -->

			<div class="draggable-handle-2"></div>
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
</div>

<style lang="postcss">
	.file-list-item {
		display: flex;
		position: relative;
		align-items: center;
		height: 32px;
		text-align: left;
		user-select: none;

		&:focus-visible {
			outline: none;
		}

		&.clickable {
			cursor: pointer;

			&:hover {
				& .draggable-handle-2 {
					opacity: 0.6;
				}
			}

			&:not(.selected):hover {
				& .file-list-item__background {
					background-color: var(--hover-bg-1);
				}
			}

			&.conflicted:not(.selected):hover {
				& .file-list-item__background {
					background-color: var(--hover-danger-bg);
				}
			}
		}

		&.conflicted {
			& .file-list-item__background {
				background-color: var(--clr-theme-danger-bg);
			}

			&.selected {
				& .file-list-item__background {
					background-color: var(--clr-theme-danger-element);
				}
			}
		}

		&.selected {
			& .file-list-item__background {
				background-color: var(--clr-bg-2);
			}
		}

		&.active.selected {
			& .file-list-item__background {
				background-color: var(--file-list-item-selected-bg);
			}
		}

		.file-list-item__content {
			display: flex;
			z-index: 2;
			position: relative;
			flex-grow: 1;
			align-items: center;
			min-width: 0; /* Allow truncation to work properly */
			padding: 0 6px 0 6px;
			overflow: hidden;
			gap: 8px;
		}

		.draggable-handle-2 {
			position: relative;
			margin: 0 -4px 0 -2px;
			opacity: 0;
			transition: opacity var(--transition-fast);
		}

		.draggable-handle-2,
		.draggable-handle-2::after,
		.draggable-handle-2::before {
			width: 2px;
			height: 2px;
			border-radius: 2px;
			background-color: var(--file-list-item-selected-fg, var(--clr-text-2));
		}

		.draggable-handle-2::before {
			position: absolute;
			top: 4px;
			content: '';
		}

		.draggable-handle-2::after {
			position: absolute;
			bottom: 4px;
			content: '';
		}

		.conflicted-icon {
			display: flex;
			margin-right: -2px;
			color: var(--file-list-item-selected-fg, var(--clr-theme-danger-element));
		}
	}

	.file-list-item__background {
		z-index: 1;
		position: absolute;
		height: 28px;
		inset: 0;
		top: 2px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		pointer-events: none;
		/* transition: background-color var(--transition-fast); */
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
