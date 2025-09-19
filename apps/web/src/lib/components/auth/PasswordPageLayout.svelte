<script lang="ts">
	import { inject } from '@gitbutler/core/context';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		children: Snippet;
	}

	let { title, children }: Props = $props();

	const routesService = inject(WEB_ROUTES_SERVICE);
</script>

<div class="service-form__page">
	<form class="service-form">
		<h1 class="text-serif-42 m-bottom-20">{title}</h1>

		{@render children()}

		<div class="text-12 service-form__footer">
			<p>
				← Back to
				<a href={routesService.loginPath()}>Login</a>
			</p>
			<p>
				Need help?
				<a
					href="https://github.com/gitbutlerapp/gitbutler/issues/new?template=BLANK_ISSUE"
					target="_blank"
					rel="noopener noreferrer"
				>
					Open a support request
				</a>
			</p>
		</div>
	</form>
</div>

<style lang="postcss">
	.service-form__page {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
	}

	.service-form {
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: 540px;
		padding: 50px 60px 40px;
		border-radius: var(--radius-xl);
		background-color: var(--clr-bg-1);
	}

	.service-form__footer {
		display: flex;
		justify-content: space-between;
		margin-top: 40px;
		color: var(--clr-text-2);
		text-align: center;

		a {
			text-decoration: underline;
			transition: color var(--transition-fast);

			&:hover {
				color: var(--clr-text-1);
			}
		}
	}

	@media (max-width: 600px) {
		.service-form {
			padding: 30px 20px 20px;
		}

		.service-form__footer {
			flex-direction: column;
			margin-top: 24px;
			gap: 8px;
		}
	}
</style>
