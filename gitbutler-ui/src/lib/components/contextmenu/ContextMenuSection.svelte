<script lang="ts">
</script>

<div class="context-menu-section">
	<slot />
</div>

<style lang="postcss">
	.context-menu-section {
		display: flex;
		flex-direction: column;
		padding: var(--size-8);
		gap: var(--size-2);
		&:not(:first-child) {
			border-top: 1px solid var(--clr-theme-container-outline-light);
		}
	}
</style>
