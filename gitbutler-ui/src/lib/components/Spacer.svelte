<script lang="ts">
	export let margin: number | undefined = 16;

	function getMargins() {
		if (margin === undefined) {
			return '';
		}

		return `margin-top: var(--space-${margin}); margin-bottom: var(--space-${margin});`;
	}
</script>

<div class="divider" style={getMargins()} />

<style lang="post-css">
	.divider {
		height: 1px;
		width: 100%;
		border-bottom: 1px solid var(--clr-theme-scale-ntrl-0);
		opacity: 0.15;
	}
</style>
