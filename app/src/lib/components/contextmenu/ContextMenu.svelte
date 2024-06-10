<div class="context-menu">
	<slot />
</div>

<style lang="postcss">
	.context-menu {
		display: flex;
		flex-direction: column;
		background: var(--clr-bg-2);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		box-shadow: var(--fx-shadow-s);
	}
</style>
