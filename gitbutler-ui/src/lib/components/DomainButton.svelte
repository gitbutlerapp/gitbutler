<script lang="ts">
	import { page } from '$app/stores';

	export let href: string;
	$: selected = $page.url.href.includes(href);
</script>

<a {href} class="domain-button text-base-14 text-semibold" class:selected>
	<slot />
</a>

<style lang="postcss">
	.domain-button {
		display: flex;
		align-items: center;
		gap: var(--space-10);
		border-radius: var(--radius-m);
		padding: var(--space-10) var(--space-8);
		color: var(--clr-theme-scale-ntrl-0);
		height: var(--space-36);
		transition: background-color var(--transition-fast);
	}

	.domain-button:hover,
	.domain-button:focus,
	.selected {
		background-color: color-mix(in srgb, transparent, var(--darken-tint-light));
	}
</style>
