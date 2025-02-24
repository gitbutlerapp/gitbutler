<script lang="ts">
	import Checkbox from '$components/Checkbox.svelte';
	import Icon from '$components/Icon.svelte';
	import FileIndent from '$components/file/FileIndent.svelte';

	interface Props {
		name: string;
		showCheckbox?: boolean;
		checked?: boolean;
		indeterminate?: boolean;
		isExpanded?: boolean;
		depth?: number;
		transparent?: boolean;
		draggable?: boolean;
		oncheck?: (
			e: Event & {
				currentTarget: EventTarget & HTMLInputElement;
			}
		) => void;
		ontoggle?: (expanded: boolean) => void;
		onclick?: (e: MouseEvent) => void;
		onkeydown?: (e: KeyboardEvent) => void;
		oncontextmenu?: (e: MouseEvent) => void;
		testId?: string;
	}

	let {
		name,
		showCheckbox,
		checked = $bindable(),
		indeterminate,
		isExpanded = true,
		depth,
		transparent,
		draggable = false,
		oncheck,
		ontoggle,
		onclick,
		onkeydown,
		oncontextmenu,
		testId
	}: Props = $props();

	let isFolderExpanded = $derived(isExpanded);
</script>

<div
	data-testid={testId}
	class="folder-list-item"
	role="presentation"
	tabindex="-1"
	class:transparent
	class:draggable
	onclick={(e) => {
		e.stopPropagation();
		onclick?.(e);
	}}
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

	<div class="folder-list-item__indicators">
		<FileIndent {depth} />

		<button
			type="button"
			aria-label="Toggle folder"
			class="folder-list-item__arrow"
			class:expanded={isFolderExpanded}
			onclick={(e) => {
				e.stopPropagation();
				isFolderExpanded = !isFolderExpanded;
				ontoggle?.(isFolderExpanded);
			}}
		>
			<svg
				width="10"
				height="6"
				viewBox="0 0 10 6"
				fill="currentColor"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path
					fill-rule="evenodd"
					clip-rule="evenodd"
					d="M5.17683 3.76259C5.0792 3.86022 4.92091 3.86022 4.82328 3.76259L1.53039 0.469698L0.469727 1.53036L3.76262 4.82325C4.44604 5.50667 5.55408 5.50667 6.23749 4.82325L9.53039 1.53036L8.46973 0.469698L5.17683 3.76259Z"
				/>
			</svg>
		</button>

		{#if showCheckbox}
			<Checkbox small bind:checked {indeterminate} onchange={oncheck} />
		{/if}

		<Icon name="folder" />
	</div>
	<p class="text-12 text-semibold truncate">{name}</p>
</div>

<style lang="postcss">
	.folder-list-item {
		display: flex;
		align-items: center;
		height: 32px;
		padding: 8px 8px 8px 14px;
		gap: 8px;
		background-color: var(--clr-bg-1);
		cursor: pointer;

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}
		&.transparent {
			background-color: transparent;
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
	}

	.folder-list-item__indicators {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--clr-text-2);
	}

	.folder-list-item__arrow {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 14px;
		height: 14px;
		margin: 0 -2px;
		transform: rotate(-90deg);
		border-radius: var(--radius-s);

		&:hover {
			color: var(--clr-text-1);
		}

		&.expanded {
			transform: rotate(0);
			transition: transform var(--transition-fast);
		}
	}
</style>
