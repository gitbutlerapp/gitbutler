<script lang="ts">
	export let name: string;
	let active = false;
	let input: HTMLInputElement;

	function activate() {
		active = true;
		setTimeout(() => input.select(), 0);
	}
</script>

{#if active}
	<input
		type="text"
		bind:this={input}
		bind:value={name}
		on:change
		title={name}
		class="branch-name-input text-base-13"
		on:dblclick|stopPropagation
		on:click={(e) => e.currentTarget.select()}
		on:blur={() => (active = false)}
		autocomplete="off"
		autocorrect="off"
		spellcheck="false"
	/>
{:else}
	<div
		role="textbox"
		tabindex="0"
		class="branch-name text-base-13"
		on:keydown={activate}
		on:click={activate}
	>
		{name}
	</div>
{/if}

<style lang="postcss">
	.branch-name,
	.branch-name-input {
		color: var(--clr-theme-scale-ntrl-0);
		padding: var(--space-4) var(--space-6);
		border-radius: var(--radius-s);
		border: 1px solid transparent;
	}
	.branch-name {
		cursor: text;
		display: inline-block;
		&:hover {
			background-color: var(--clr-theme-container-pale);
		}
	}
	.branch-name-input {
		width: 100%;
		background-color: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-container-outline-light);
	}
</style>
