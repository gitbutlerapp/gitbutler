<script lang="ts">
	import { inject } from '@gitbutler/core/context';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';

	interface Props {
		ghost?: boolean;
	}

	const { ghost }: Props = $props();

	const routes = inject(WEB_ROUTES_SERVICE);
</script>

<a href={routes.homePath()} class="logo" class:ghost aria-label="main nav" title="Home">
	{#if !ghost}
		<span class="logo-text">GitButler</span>
		<div class="logo-mark">
			<svg
				width="100%"
				height="100%"
				viewBox="0 0 23 22"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path d="M0 22V0L11.4819 9.63333L23 0V22L11.4819 12.4L0 22Z" fill="var(--clr-text-1)" />
			</svg>
		</div>
	{:else}
		<div class="logo-ghost">
			<svg width="100%" height="100%" viewBox="0 0 23 22" xmlns="http://www.w3.org/2000/svg">
				<path d="M0 22V0L11.4819 9.63333L23 0V22L11.4819 12.4L0 22Z" />
			</svg>
		</div>
	{/if}
</a>

<style lang="postcss">
	.logo {
		display: flex;
		align-items: center;
		gap: 10px;
		text-decoration: none;
	}

	.logo-text {
		color: var(--clr-text-1);
		font-size: 40px;
		font-family: var(--fontfamily-accent);
	}

	.logo-mark,
	.logo-ghost {
		display: flex;
		width: 28px;
		height: 100%;
		margin-top: 5px;
	}

	.logo-ghost {
		display: flex;
		width: 28px;
		height: 28px;
		margin-top: 5px;
		fill: var(--clr-bg-2);
		stroke: var(--clr-text-3);
		stroke-width: 1;
		transition:
			stroke var(--transition-fast),
			fill var(--transition-medium);

		&:hover {
			stroke: var(--clr-text-1);
			fill: var(--clr-text-1);
		}
	}
</style>
