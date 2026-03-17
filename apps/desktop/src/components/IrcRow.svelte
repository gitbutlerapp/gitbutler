<script lang="ts">
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Icon } from "@gitbutler/ui";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import { slide } from "svelte/transition";

	type Props = {
		stackId?: string;
		channel?: string;
		selected?: boolean;
		onclick?: () => void;
	};

	const { stackId, channel, selected = false, onclick }: Props = $props();

	const uiState = inject(UI_STATE);
	const laneState = $derived(stackId ? uiState.lane(stackId) : undefined);

	let active = $state(false);

	function toggleSelection() {
		if (!laneState) return;
		laneState.selection.set(selected ? undefined : { irc: true, previewOpen: true });
		onclick?.();
	}
</script>

<button
	type="button"
	class="irc-row"
	class:selected
	class:active
	onclick={toggleSelection}
	use:focusable={{
		onAction: toggleSelection,
		onActive: (value) => (active = value),
		focusable: true,
	}}
>
	{#if selected}
		<div
			class="indicator"
			class:selected
			class:active
			in:slide={{ axis: "x", duration: 150 }}
		></div>
	{/if}

	<Icon name="chat" />
	<h3 class="text-13 text-semibold truncate irc-row__title">{channel ?? "IRC"}</h3>
</button>

<style lang="postcss">
	.irc-row {
		display: flex;
		position: relative;
		align-items: center;
		width: 100%;
		height: 44px;
		padding: 0 12px;
		padding-left: 14px;
		gap: 8px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-muted);
		text-align: left;

		transition: background-color var(--transition-fast);

		&.active.selected,
		&[type="button"]:hover {
			background-color: var(--clr-bg-1);
		}
	}

	.irc-row__title {
		flex: 1;
		color: var(--clr-text-1);
	}

	.indicator {
		position: absolute;
		top: 50%;
		left: 0;
		width: 4px;
		height: 45%;
		transform: translateY(-50%);
		border-radius: 0 var(--radius-ml) var(--radius-ml) 0;
		background-color: var(--clr-theme-ntrl-element);
		transition: transform var(--transition-fast);
	}
</style>
