<script lang="ts">
	import Checkbox from '$lib/Checkbox.svelte';
	import Icon from '$lib/Icon.svelte';
	import FileIndent from '$lib/file/FileIndent.svelte';

	interface Props {
		name: string;
		showCheckbox?: boolean;
		checked?: boolean;
		indeterminate?: boolean;
		isExpanded?: boolean;
		depth?: number;
		oncheck?: (
			e: Event & {
				currentTarget: EventTarget & HTMLInputElement;
			}
		) => void;
		ontoggle?: (expanded: boolean) => void;
		onclick?: (e: MouseEvent) => void;
		onkeydown?: (e: KeyboardEvent) => void;
	}

	let {
		name,
		showCheckbox,
		checked = $bindable(),
		indeterminate,
		isExpanded = true,
		depth,
		oncheck,
		ontoggle,
		onclick,
		onkeydown
	}: Props = $props();

	let isFolderExpanded = $state(isExpanded);

	$effect(() => {
		isFolderExpanded = isExpanded;
	});
</script>

<div
	class="folder-list-item"
	role="presentation"
	tabindex="-1"
	onclick={(e) => {
		e.stopPropagation();
		onclick?.(e);
	}}
	{onkeydown}
>
	<div class="folder-list-item__indicators">
		<FileIndent {depth} />

		<button
			type="button"
			aria-label="Toggle folder"
			class="folder-list-item__arrow focus-state"
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
	<p class="text-12 text-semibold">{name}</p>
</div>

<style lang="postcss">
	.folder-list-item {
		cursor: pointer;
		display: flex;
		align-items: center;
		gap: 8px;
		height: 32px;
		padding: 8px 8px 8px 14px;

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}
	}

	.folder-list-item__indicators {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--clr-text-2);
		height: 100%;
	}

	.folder-list-item__arrow {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 14px;
		height: 14px;
		border-radius: var(--radius-s);
		margin: 0 -2px;
		transform: rotate(-90deg);

		&:hover {
			color: var(--clr-text-1);
		}

		&.expanded {
			transform: rotate(0);
			transition: transform var(--transition-fast);
		}
	}
</style>
