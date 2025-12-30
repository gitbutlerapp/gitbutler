<script lang="ts">
	import { env } from '$env/dynamic/public';

	interface Props {
		mode?: 'signin' | 'signup';
	}

	let { mode = 'signin' }: Props = $props();

	const actionText = mode === 'signup' ? 'Sign up' : 'Sign in';
</script>

<div class="auth-form__social">
	<div class="auth-form__social-title">
		<span class="text-12"> {mode === 'signup' ? 'Or sign up with' : 'Or sign in with'} </span>
	</div>
	<div class="oauth-buttons">
		{#snippet oauthButton(provider: 'github' | 'google' | 'gitea')}

			{@const config = {
				github: {
					endpoint: 'auth/github',
					title: `${actionText} with GitHub`,
					label: 'GitHub',
					svg: `<svg class="oauth-logo" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
					<path fill-rule="evenodd" clip-rule="evenodd" d="M48.854 0C21.839 0 0 22 0 49.217c0 21.756 13.993 40.172 33.405 46.69 2.427.49 3.316-1.059 3.316-2.362 0-1.141-.08-5.052-.08-9.127-13.59 2.934-16.42-5.867-16.42-5.867-2.184-5.704-5.42-7.17-5.42-7.17-4.448-3.015.324-3.015.324-3.015 4.934.326 7.523 5.052 7.523 5.052 4.367 7.496 11.404 5.378 14.235 4.074.404-3.178 1.699-5.378 3.074-6.6-10.839-1.141-22.243-5.378-22.243-24.283 0-5.378 1.94-9.778 5.014-13.2-.485-1.222-2.184-6.275.486-13.038 0 0 4.125-1.304 13.426 5.052a46.97 46.97 0 0 1 12.214-1.63c4.125 0 8.33.571 12.213 1.63 9.302-6.356 13.427-5.052 13.427-5.052 2.67 6.763.97 11.816.485 13.038 3.155 3.422 5.015 7.822 5.015 13.2 0 18.905-11.404 23.06-22.324 24.283 1.78 1.548 3.316 4.481 3.316 9.126 0 6.6-.08 11.897-.08 13.526 0 1.304.89 2.853 3.316 2.364 19.412-6.52 33.405-24.935 33.405-46.691C97.707 22 75.788 0 48.854 0z" fill="currentColor"/>
				</svg>`
				},
				google: {
					endpoint: 'auth/google_oauth2',
					title: `${actionText} with Google`,
					label: 'Google',
					svg: `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class="oauth-logo">
					<path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" fill="#4285f4"></path>
					<path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" fill="#34a853"></path>
					<path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" fill="#fbbc05"></path>
					<path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" fill="#ea4335"></path>
				</svg>`
				},
				gitea: {
					endpoint: 'auth/gitea',
					title: `${actionText} with Gitea`,
					label: 'Gitea',
					svg: `<svg role="img" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><title>Gitea</title><path d="M12 0C5.372 0 0 5.373 0 12s5.372 12 12 12 12-5.373 12-12S18.628 0 12 0zm.557 3.303a2.43 2.43 0 0 1 1.83 1.487c.21 1.134-1.29 2.083-2.126 1.42-.835-.662.086-2.906.296-2.906zm-6.837 3.12c.507-1.168 2.09-1.173 2.37.155.074.354-.236.72-.513.886-1.127.674-2.107-.45-1.857-1.04zm11.332 1.34c.823-.427 1.636.56 1.165 1.38-.48.835-1.805.215-1.745-.615.03-.418.293-.65.58-.765zM4.09 10.3c.69-.974 2.193-.572 2.175.602-.007.453-.3.844-.664 1.053-.9.516-1.928-.96-1.512-1.655zm14.755 1.76c.742.274 1.114 1.258.46 1.777-.732.58-1.76-.324-1.42-1.107.173-.398.578-.654.96-.67zm-14.28 2.375c.29-.684 1.167-.84 1.683-.356.517.485.347 1.373-.32 1.63-.667.256-1.503-.59-1.363-1.274zm13.1 2.333c.334.46.242 1.23-.29 1.492-.534.262-1.1-.303-1.002-.855.05-.282.268-.507.525-.595.257-.088.542-.09.767-.042zm-5.632.748c1.378-.014 2.753.045 4.12.2l2.368.618c.28.188.423.54.43.86.046 2.05-1.996 2.46-3.21 2.44-1.385-.022-2.772-.097-4.158-.157-1.386.06-2.772.135-4.157.157-1.214.02-3.256-.39-3.21-2.44.007-.32.15-.672.43-.86l2.37-.62c1.365-.154 2.74-.213 4.118-.2zm-3.87 1.05c-.097.55-.662 1.116-1.196.855-.533-.262-.624-1.032-.29-1.492.224-.047.51-.046.767.042.257.088.475.313.525.595.093.552-.472 1.115-1.002.855.53.26 1.1-.303 1.002-.855-.098-.553-.663-1.116-1.197-.855-.533.262-.624 1.032-.29 1.492.334.46.903.374 1.196-.042zm6.27-.135c-.443.515-1.41.206-1.306-.48.052-.34.33-.603.655-.647.78-.106 1.094.612.65 1.127z"/></svg>`
				}
			}[provider]}

			<a
				class="oauth-btn"
				href={`${env.PUBLIC_APP_HOST}${config.endpoint}?origin=${encodeURIComponent(window.location.href)}`}
				title={config.title}
				aria-label={config.title}
			>
				<div class="oauth-logo">
					{@html config.svg}
				</div>
				<span class="text-14 text-semibold">{config.label}</span>
			</a>
		{/snippet}

		{@render oauthButton('github')}
		{@render oauthButton('google')}
		{@render oauthButton('gitea')}
	</div>
</div>

<style lang="postcss">
	.oauth-buttons {
		display: flex;
		gap: 8px;
	}

	.oauth-btn {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		padding: 8px;
		padding-right: 14px;
		gap: 8px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		color: var(--clr-text-1);

		&:hover {
			background-color: var(--hover-bg-1);
		}
	}

	.oauth-logo {
		width: 16px;
		height: 16px;
	}

	.auth-form__social {
		display: flex;
		flex-direction: column;
		margin-top: 24px;
	}

	.auth-form__social-title {
		display: flex;
		justify-content: center;
		margin-bottom: 16px;
		color: var(--clr-text-2);

		span {
			margin: 0 12px;
		}

		&::before,
		&::after {
			flex: 1;
			margin: auto 0;
			border-bottom: 1px solid var(--clr-border-2);
			content: '';
		}
	}
</style>
