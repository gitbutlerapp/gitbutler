<script lang="ts">
	import InfoMessage from './InfoMessage.svelte';

	// Number of events received.
	let count = $state(0);

	/**
	 * Listens for custom events sent by vite, defined in `vite.config.ts`.
	 * See: https://vite.dev/guide/api-plugin.html#typescript-for-custom-events
	 */
	import.meta.hot?.on('gb:reload', () => {
		count++;
	});
</script>

{#if count > 0}
	<div class="reload-warning">
		<InfoMessage style="warning">
			{#snippet title()}
				Full reload pending
			{/snippet}
			{#snippet content()}
				Detected {count} events that require reloading this page.
			{/snippet}
		</InfoMessage>
	</div>
{/if}

<style lang="postcss">
	.reload-warning {
		position: absolute;
		bottom: 24px;
		left: 50%;
		transform: translateX(-50%);
		z-index: var(--z-lifted);
	}
</style>
