<script lang="ts">
	import { page } from '$app/stores';

	export let href: string;
	$: selected = $page.url.href.includes(href);
</script>

<a
	{href}
	class="domain-button text-base-14 text-semibold"
	style:background-color={selected ? 'var(--clr-theme-container-pale)' : undefined}
>
	<slot />
</a>

<style lang="postcss">
	.domain-button {
		display: flex;
		align-items: center;
		gap: var(--space-10);
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-container-light);
		padding: var(--space-10) var(--space-8);
		transition: background-color var(--transition-fast);
		color: var(--clr-theme-scale-ntrl-0);
		height: var(--space-36);

		&:hover,
		&:focus {
			background-color: var(--clr-theme-container-pale);
		}
	}
</style>
