<script lang="ts">
</script>

<div class="context-menu-section">
	<slot />
</div>

<style lang="postcss">
	.context-menu-section {
		display: flex;
		flex-direction: column;
		padding: 8px;
		gap: 2px;
		&:not(:first-child) {
			border-top: 1px solid var(--clr-border-2);
		}
	}
</style>
