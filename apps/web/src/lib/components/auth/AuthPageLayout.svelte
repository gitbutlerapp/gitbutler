<script lang="ts">
	import walkininSvg from '$lib/assets/splash-illustrations/walkin.svg?raw';
	import OAuthButtons from '$lib/components/login/OAuthButtons.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		title: string;
		subtitle?: string;
		oauthText?: string;
		oauthMode?: 'signin' | 'signup';
		bottomLinkText?: string;
		bottomLinkHref?: string;
		bottomLinkLabel?: string;
		children: Snippet;
	}

	let {
		title,
		subtitle,
		oauthText = 'Or continue with',
		oauthMode = 'signin',
		bottomLinkText,
		bottomLinkHref,
		bottomLinkLabel,
		children
	}: Props = $props();
</script>

<div class="auth-page">
	<div class="auth-form__container">
		<div class="auth-form">
			<h1 class="text-serif-42 auth-form__title">
				<i>{title}</i>
				{#if subtitle}
					{subtitle}
				{/if}
			</h1>

			{@render children()}

			<div class="auth-form__social">
				<div class="auth-form__social-title">
					<span class="text-12">{oauthText}</span>
				</div>

				<OAuthButtons mode={oauthMode} />
			</div>

			{#if bottomLinkText && bottomLinkHref && bottomLinkLabel}
				<div class="text-12 auth-bottom-link">
					<p>{bottomLinkText} <a href={bottomLinkHref}>{bottomLinkLabel}</a></p>
				</div>
			{/if}
		</div>

		<div class="auth-form__illustration">
			{@html walkininSvg}
		</div>
	</div>
</div>

<style lang="postcss">
	.auth-page {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
	}

	.auth-form__container {
		display: flex;
		width: 100%;
		max-width: 1048px;
		overflow: hidden;
		border-radius: var(--radius-xl);
	}

	.auth-form {
		display: flex;
		flex: 4;
		flex-direction: column;
		width: 100%;
		padding: 50px 80px 30px;
		background-color: var(--clr-bg-1);
	}

	.auth-form__title {
		margin-bottom: 40px;
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

	.auth-bottom-link {
		display: flex;
		align-items: center;
		justify-content: center;
		margin-top: 40px;
		gap: 4px;
		color: var(--clr-text-2);

		a {
			text-decoration: underline;

			&:hover {
				color: var(--clr-text-1);
			}
		}
	}

	.auth-form__illustration {
		display: flex;
		flex: 4;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 32px;
		background-color: var(--clr-illustration-bg);

		:global(svg) {
			max-width: 400px;
		}
	}

	@media (max-width: 1020px) {
		.auth-form {
			flex: 5;
			padding: 40px 40px 20px;
		}
	}

	@media (max-width: 840px) {
		.auth-form__illustration {
			display: none;
		}

		.auth-form__title {
			margin-bottom: 24px;
		}
	}

	@media (max-width: 480px) {
		.auth-form {
			padding: 30px 20px 20px;
		}
	}
</style>
