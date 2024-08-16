<script lang="ts">
	import { onMount } from 'svelte';

	interface Props {
		name: string;
		disabled?: boolean;
		onchange?: (value: string) => void;
	}

	let { name, disabled = false, onchange }: Props = $props();

	let inputEl: HTMLInputElement;
	let initialName = name;

	onMount(() => {
		inputEl.style.width = `${name.length}ch`;
	});
</script>

<input
	type="text"
	{disabled}
	bind:this={inputEl}
	bind:value={name}
	onchange={(e) => onchange?.(e.currentTarget.value.trim())}
	onkeypress={(e) => {
		inputEl.style.width = `${e.currentTarget.value.trim().length}ch`;
	}}
	title={name}
	class="branch-name-input text-14 text-bold"
	ondblclick={(e) => e.stopPropagation()}
	onclick={(e) => {
		e.stopPropagation();
		inputEl.focus();
	}}
	onblur={() => {
		if (name === '') name = initialName;
	}}
	onfocus={() => {
		initialName = name;
	}}
	onkeydown={(e) => {
		if (e.key === 'Enter' || e.key === 'Escape') {
			inputEl.blur();
		}
	}}
	autocomplete="off"
	autocorrect="off"
	spellcheck="false"
/>

<style lang="postcss">
	.branch-name-input {
		min-width: 8ch;
		height: 20px;
		padding: 2px 4px;
		border: 1px solid transparent;

		text-overflow: ellipsis;
		white-space: nowrap;
		overflow: hidden;

		max-width: 100%;
		border-radius: var(--radius-s);
		color: var(--clr-scale-ntrl-0);
		background-color: var(--clr-bg-1);
		outline: none;

		/* not readonly */
		&:not([disabled]):hover {
			background-color: var(--clr-bg-2);
		}

		&:not([disabled]):focus {
			outline: none;
			background-color: var(--clr-bg-2);
			border-color: var(--clr-border-2);
		}
	}
</style>
