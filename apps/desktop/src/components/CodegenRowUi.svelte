<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import { Icon } from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { slide } from 'svelte/transition';
	import type { ClaudeStatus } from '$lib/codegen/types';
	import type { DropzoneHandler } from '$lib/dragging/handler';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	type Props = {
		branchName: string;
		selected?: boolean;
		status?: ClaudeStatus;
		handlers?: DropzoneHandler[];
		onselect?: () => void;
		text: string | undefined;
		todos?: {
			total: number;
			completed: number;
		};
	};

	const { handlers, selected, status, text, todos, onselect }: Props = $props();

	let active = $state(false);

	function getCurrentIconName(): keyof typeof iconsJson {
		if (status === 'running' || status === 'compacting') {
			return 'spinner';
		}
		return 'ai';
	}

	function toggleSelection() {
		onselect?.();
	}
</script>

<Dropzone handlers={handlers || []}>
	{#snippet overlay({ hovered, activated })}
		<CardOverlay {hovered} {activated} label="Reference" />
	{/snippet}
	<button
		type="button"
		class="codegen-row"
		class:selected
		class:active
		onclick={toggleSelection}
		use:focusable={{
			onAction: toggleSelection,
			onActive: (value) => (active = value),
			focusable: true
		}}
	>
		{#if selected}
			<div
				class="indicator"
				class:selected
				class:active
				in:slide={{ axis: 'x', duration: 150 }}
			></div>
		{/if}

		<Icon name={getCurrentIconName()} color="var(--clr-theme-purp-element)" />
		<h3 class="text-13 text-semibold truncate codegen-row__title">{text}</h3>

		{#if todos && todos.total > 1}
			<span class="text-12 codegen-row__todos">Todos ({todos.completed}/{todos.total})</span>

			{#if todos.completed === todos.total}
				<Icon name="success-outline" color="success" />
			{/if}
		{/if}
	</button>
</Dropzone>

<style lang="postcss">
	.codegen-row {
		display: flex;
		position: relative;
		width: 100%;
		padding: 12px;
		padding-left: 14px;
		gap: 8px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-theme-purp-bg);
		text-align: left;
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-theme-purp-bg-muted);
		}

		/* Selected in focus */
		&.active.selected {
			background-color: var(--clr-theme-purp-bg-muted);
		}
	}

	.codegen-row__title {
		flex: 1;
		color: var(--clr-theme-purp-on-soft);
	}

	.codegen-row__todos {
		flex-shrink: 0;
		color: var(--clr-theme-purp-on-soft);
		opacity: 0.7;
	}

	.indicator {
		position: absolute;
		top: 50%;
		left: 0;
		width: 4px;
		height: 45%;
		transform: translateY(-50%);
		border-radius: 0 var(--radius-ml) var(--radius-ml) 0;
		background-color: var(--clr-theme-purp-element);
		transition: transform var(--transition-fast);

		&.active {
			background-color: var(--clr-theme-purp-element);
		}
	}
</style>
