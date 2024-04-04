<script lang="ts">
	export let disabled = false;
</script>

<div class="section" class:disabled>
	<div class="header">
		<p class="text-base-14 text-semibold title"><slot name="label" /></p>
		<p class="text-base-13 description"><slot name="description" /></p>
	</div>
	<div class="text-base-13 content">
		<slot />
	</div>
</div>

<style lang="postcss">
	.section {
		display: flex;
		flex-direction: column;
		gap: var(--size-16);
	}
	.header {
		display: flex;
		flex-direction: column;
		gap: var(--size-4);
	}
	.title {
		color: var(--clr-scale-ntrl-40);
	}
	.description {
		color: var(--clr-scale-ntrl-50);
		padding: var(--size-4) 0;
	}
	.content {
		display: flex;
		align-items: center;
		color: var(--clr-scale-ntrl-40);
		border-radius: var(--radius-m);
		padding: 0 var(--size-16);
		gap: var(--size-8);
	}
	.disabled {
		pointer-events: none;
		opacity: 0.6;
	}
</style>
