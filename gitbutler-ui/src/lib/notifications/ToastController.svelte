<script lang="ts" context="module">
</script>

<script lang="ts">
	import InfoMessage from '../components/InfoMessage.svelte';
	import { dismissToast, toastStore } from '$lib/notifications/toasts';
	import { slide } from 'svelte/transition';
</script>

<div class="toast-controller">
	{#each $toastStore as toast (toast.id)}
		<div transition:slide={{ duration: 170 }}>
			<InfoMessage
				title={toast.title}
				style={toast.style ?? 'neutral'}
				secondary="Dismiss"
				on:secondary={() => dismissToast(toast.id)}
				shadow>{toast.message}</InfoMessage
			>
		</div>
	{/each}
</div>

<style lang="postcss">
	.toast-controller {
		position: absolute;
		display: flex;
		flex-direction: column;
		bottom: var(--space-20);
		right: var(--space-20);
		gap: var(--space-8);
		z-index: 50;
	}
</style>
