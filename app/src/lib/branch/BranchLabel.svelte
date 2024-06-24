<script lang="ts">
	import { useResize } from '$lib/utils/useResize';
	import { createEventDispatcher } from 'svelte';

	export let name: string;
	export let disabled = false;

	let inputEl: HTMLInputElement;
	let initialName = name;

	let mesureEl: HTMLSpanElement;
	let inputWidth: string | undefined;

	const dispatch = createEventDispatcher<{
		change: { name: string };
	}>();
</script>

<span
	use:useResize={(e) => {
		inputWidth = `${Math.round(e.frame.width)}px`;
	}}
	class="branch-name-mesure-el text-base-14 text-bold"
	bind:this={mesureEl}>{name}</span
>
<input
	type="text"
	{disabled}
	bind:this={inputEl}
	bind:value={name}
	on:change={(e) => dispatch('change', { name: e.currentTarget.value.trim() })}
	title={name}
	class="branch-name-input text-base-14 text-bold"
	on:dblclick|stopPropagation
	on:click|stopPropagation={() => {
		inputEl.focus();
	}}
	on:blur={() => {
		if (name === '') name = initialName;
	}}
	on:focus={() => {
		initialName = name;
	}}
	on:keydown={(e) => {
		if (e.key === 'Enter' || e.key === 'Escape') {
			inputEl.blur();
		}
	}}
	autocomplete="off"
	autocorrect="off"
	spellcheck="false"
	style:width={inputWidth}
/>

<style lang="postcss">
	.branch-name-mesure-el,
	.branch-name-input {
		min-width: 44px;
		height: 20px;
		padding: 2px 4px;
		border: 1px solid transparent;
	}
	.branch-name-mesure-el {
		pointer-events: none;
		visibility: hidden;
		border: 2px solid transparent;
		color: black;
		position: fixed;
		display: inline-block;
		visibility: hidden;
		white-space: pre;
	}
	.branch-name-input {
		text-overflow: ellipsis;
		white-space: nowrap;
		overflow: hidden;

		max-width: 100%;
		width: 100%;
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
