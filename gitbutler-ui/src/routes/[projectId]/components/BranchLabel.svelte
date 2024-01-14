<script lang="ts">
	export let name: string;
	let inputActive = false;
	let label: HTMLDivElement;
	let input: HTMLInputElement;

	function activateInput() {
		inputActive = true;
		setTimeout(() => input.select(), 0);
	}

	let initialName = name;

	let mesureEl: HTMLSpanElement;
	let inputPadding = 10;
	let inputWidth = 0;

	$: {
		if (mesureEl) {
			inputWidth = mesureEl.getBoundingClientRect().width + inputPadding;
		}
	}
</script>

{#if inputActive}
	<span class="branch-name-mesure-el text-base-13" bind:this={mesureEl}>{name}</span>
	<input
		type="text"
		bind:this={input}
		bind:value={name}
		on:change
		on:input={() => {
			if (input.value.length > 0) {
				inputWidth = mesureEl.getBoundingClientRect().width + inputPadding;
			} else {
				inputWidth = 0;
			}
		}}
		title={name}
		class="branch-name-input text-base-13"
		on:dblclick|stopPropagation
		on:blur={() => (inputActive = false)}
		on:keydown={(e) => {
			if (e.key == 'Enter') {
				inputActive = false;
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
		style={`width: ${inputWidth}px`}
	/>
{:else}
	<div
		bind:this={label}
		role="textbox"
		tabindex="0"
		class="branch-name text-base-13 truncate"
		on:keydown={(e) => e.key == 'Enter' && activateInput()}
		on:click={activateInput}
	>
		{name}
	</div>
{/if}

<style lang="postcss">
	.branch-name,
	.branch-name-input {
		height: var(--space-20);
		pointer-events: auto;
		color: var(--clr-theme-scale-ntrl-0);
		padding: var(--space-2) var(--space-4);
		border-radius: var(--radius-s);
		border: 1px solid transparent;
	}
	.branch-name {
		cursor: text;
		display: inline-block;
		transition: background-color var(--transition-fast);
		&:hover,
		&:focus {
			background-color: var(--clr-theme-container-pale);
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
		/* width: 100%; */
		min-width: 1rem;
		max-width: 100%;
		background-color: var(--clr-theme-container-light);
		/* border: 1px solid var(--clr-theme-container-outline-light); */
		outline: none;
		&:focus {
			outline: none;
			/* border-color: var(--clr-theme-container-outline-light); */
			background-color: var(--clr-theme-container-pale);
		}
	}
</style>
