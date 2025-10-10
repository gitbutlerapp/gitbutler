<script lang="ts">
	import GitbutlerLogoLink from '$lib/components/GitbutlerLogoLink.svelte';
	import HeaderAuthSection from '$lib/components/HeaderAuthSection.svelte';

	interface NavigationProps {
		markOnly?: boolean;
		breadcrumbs?: { label: string; href: string }[];
	}

	const { markOnly, breadcrumbs }: NavigationProps = $props();
</script>

<nav class="navigation" class:justify-center={markOnly} class:justify-between={!markOnly}>
	{#if markOnly}
		<GitbutlerLogoLink markOnly />
	{:else}
		<div class="navigation__left">
			<GitbutlerLogoLink markOnly />
			{#if breadcrumbs && breadcrumbs.length > 0}
				<div class="breadcrumbs">
					{#each breadcrumbs as crumb, index (crumb.label)}
						<svg
							width="10"
							height="16"
							viewBox="0 0 10 16"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path d="M9 0.5L1 15.5" stroke="var(--clr-text-3)" />
						</svg>

						{#if index < breadcrumbs.length - 1}
							<a href={crumb.href} class="text-15 text-bold link">
								{crumb.label}
							</a>
						{:else}
							<span class="text-15 text-bold">{crumb.label}</span>
						{/if}
					{/each}
				</div>
			{/if}
		</div>

		<HeaderAuthSection />
	{/if}
</nav>

<style lang="postcss">
	.navigation {
		display: flex;
		grid-column: full-start / full-end;
		align-items: center;
		width: 100%;
		padding: 20px 0 24px;
		gap: 16px;
	}

	.navigation__left {
		display: flex;
		align-items: center;
		gap: 16px;
	}

	.breadcrumbs {
		display: flex;
		align-items: center;
		gap: 10px;

		& .link {
			color: var(--clr-text-2);
			transition: color var(--transition-fast);

			&:hover {
				color: var(--clr-text-1);
			}
		}
	}
</style>
