<script lang="ts">
	import { page } from '$app/stores';

	export let href: string;
	export let label: string;
	$: selected = $page.url.href.includes(href);
</script>

<a
	{href}
	class="domain-button"
	style:background-color={selected ? 'var(--clr-theme-container-pale)' : undefined}
>
	<slot />
	<span class="text-base-14 text-semibold label">{label}</span>
</a>

<style lang="postcss">
	.domain-button {
		display: flex;
		align-items: center;
		gap: var(--space-10);
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-container-light);
		padding: var(--space-10);
		transition: background-color var(--transition-fast);

		&:hover,
		&:focus {
			background-color: var(--clr-theme-container-pale);
		}
	}

	.label {
		color: var(--clr-theme-scale-ntrl-0);
	}
</style>
