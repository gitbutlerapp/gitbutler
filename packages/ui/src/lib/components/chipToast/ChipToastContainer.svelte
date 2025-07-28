<script lang="ts">
	import ChipToast from '$components/chipToast/ChipToast.svelte';
	import { chipToasts } from '$components/chipToast/chipToastStore';
	import { flip } from 'svelte/animate';
</script>

<div class="chip-toast-container">
	{#each $chipToasts as chipToast (chipToast.id)}
		<div animate:flip={{ duration: 200 }}>
			<ChipToast message={chipToast.message} type={chipToast.type} />
		</div>
	{/each}
</div>

<style>
	.chip-toast-container {
		display: flex;
		z-index: 9999;
		position: fixed;
		bottom: 20px;
		left: 50%;
		flex-direction: column;
		gap: 8px;
		transform: translateX(-50%);
		pointer-events: none;
	}

	.chip-toast-container > * {
		pointer-events: auto;
	}

	/* Responsive adjustments */
	@media (max-width: 480px) {
		.chip-toast-container {
			right: 16px;
			left: 16px;
			transform: none;
		}
	}
</style>
