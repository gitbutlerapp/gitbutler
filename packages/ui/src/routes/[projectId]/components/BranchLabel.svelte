<script lang="ts">
	export let name: string;
	let inputActive = false;
	let label: HTMLDivElement;
	let input: HTMLInputElement;

	function activateInput() {
		inputActive = true;
		setTimeout(() => input.select(), 0);
	}
</script>

{#if inputActive}
	<input
		type="text"
		bind:this={input}
		bind:value={name}
		on:change
		title={name}
		class="branch-name-input text-base-13"
		on:dblclick|stopPropagation
		on:click={(e) => e.currentTarget.select()}
		on:blur={() => (inputActive = false)}
		on:keydown={(e) => {
			if (e.key == 'Enter') {
				inputActive = false;
				setTimeout(() => label.focus(), 0);
			}
		}}
		autocomplete="off"
		autocorrect="off"
		spellcheck="false"
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
		height: var(--size-btn-m);
		pointer-events: auto;
		color: var(--clr-theme-scale-ntrl-0);
		padding: var(--space-6);
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
	.branch-name-input {
		width: 100%;
		/* background-color: var(--clr-theme-container-pale); */
		border: 1px solid var(--clr-theme-container-outline-light);
		&:focus {
			outline: none;
			border-color: var(--clr-theme-container-outline-light);
		}
	}
</style>
