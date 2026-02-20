<!--
	Example component showing how to use the NetworkStatusService
-->
<script lang="ts">
	import { NETWORK_STATUS_SERVICE } from '$lib/network/networkStatusService.svelte';
	import { inject } from '@gitbutler/core/context';

	const networkStatusService = inject(NETWORK_STATUS_SERVICE);

	// Access the reactive online status
	const isOnline = $derived(networkStatusService.online.current);
</script>

{#if !isOnline}
	<div class="network-status-banner">
		<span>You are currently offline</span>
	</div>
{/if}

<style>
	.network-status-banner {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 8px;
		background-color: var(--clr-bg-warn);
		color: var(--clr-text-1);
		font-size: 12px;
	}
</style>
