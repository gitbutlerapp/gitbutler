<script lang="ts">
	export let name: string;
	export let disabled = false;
	let inputActive = false;
	let label: HTMLDivElement;
	let input: HTMLInputElement;

	function activateInput() {
		if (disabled) return;
		inputActive = true;
		setTimeout(() => input.select(), 0);
	}

	let initialName = name;

	let mesureEl: HTMLSpanElement;
	let inputWidth = 0;

	$: {
		if (mesureEl) {
			inputWidth = mesureEl.getBoundingClientRect().width;
		}
	}
</script>

{#if inputActive}
	<span class="branch-name-mesure-el text-base-13 text-bold" bind:this={mesureEl}>{name}</span>
	<input
		type="text"
		{disabled}
		bind:this={input}
		bind:value={name}
		on:change
		on:input={() => {
			if (input.value.length > 0) {
				inputWidth = mesureEl.getBoundingClientRect().width;
			} else {
				inputWidth = 0;
			}
		}}
		title={name}
		class="branch-name-input text-base-13 text-bold"
		on:dblclick|stopPropagation
		on:blur={() => (inputActive = false)}
		on:keydown={(e) => {
			if (e.key == 'Enter') {
				// Unmount input field asynchronously to ensure on:change gets executed.
				setTimeout(() => (inputActive = false), 0);
				setTimeout(() => label.focus(), 0);
			}

			if (e.key == 'Escape') {
				inputActive = false;
				name = initialName;
				setTimeout(() => label.focus(), 0);
			}
		}}
		autocomplete="off"
		autocorrect="off"
		spellcheck="false"
		style={`width: calc(${inputWidth}px + var(--size-12))`}
	/>
{:else}
	<div
		bind:this={label}
		role="textbox"
		tabindex="0"
		class="branch-name text-base-13 text-bold truncate"
		on:keydown={(e) => e.key == 'Enter' && activateInput()}
		on:mousedown={activateInput}
	>
		{name}
	</div>
{/if}

<style lang="postcss">
	.branch-name,
	.branch-name-input {
		height: var(--size-20);
		pointer-events: auto;
		color: var(--clr-scale-ntrl-0);
		padding: var(--size-2) var(--size-4);
		border-radius: var(--radius-s);
		border: 1px solid transparent;
	}
	.branch-name {
		cursor: text;
		display: inline-block;
		transition: background-color var(--transition-fast);
		&:hover,
		&:focus {
			background-color: color-mix(
				in srgb,
				var(--clr-theme-container-light),
				var(--darken-tint-light)
			);
			outline: none;
		}
	}
	.branch-name-mesure-el {
		visibility: hidden;
		position: absolute;
		display: inline-block;
		visibility: hidden;
		white-space: pre;
	}
	.branch-name-input {
		min-width: 1rem;
		max-width: 100%;
		background-color: var(--clr-container-light);
		outline: none;
		&:focus {
			outline: none;
			background-color: color-mix(
				in srgb,
				var(--clr-theme-container-light),
				var(--darken-tint-light)
			);
		}
	}
</style>
