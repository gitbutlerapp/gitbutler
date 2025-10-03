<script lang="ts">
	import { inject } from '@gitbutler/core/context';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';

	interface Props {
		markOnly?: boolean;
	}

	const { markOnly }: Props = $props();

	const routes = inject(WEB_ROUTES_SERVICE);
</script>

<a href={routes.homePath()} class="logo" aria-label="main nav" title="Go to Home">
	{#if !markOnly}
		<span class="logo-text">GitButler</span>
	{/if}
	<div class="logo-mark" class:mark-only={markOnly}>
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

	.logo-mark {
		display: flex;
		width: 28px;
		height: 100%;

		&:not(.mark-only) {
			margin-top: 5px;
		}
	}
</style>
