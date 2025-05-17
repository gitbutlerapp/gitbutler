<script lang="ts">
	import AccountLink from '$components/AccountLink.svelte';
	import gbLogoSvg from '$lib/assets/gb-logo.svg?raw';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { User } from '$lib/user/user';
	import { openExternalUrl } from '$lib/utils/url';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { type Snippet } from 'svelte';

	interface Props {
		showLinks?: boolean;
		img?: string;
		children?: Snippet;
		title?: Snippet;
		description?: Snippet;
	}

	const { showLinks = true, img, children, title, description }: Props = $props();

	const user = getContextStore(User);

	const settingsService = getContext(SettingsService);
	const appSettings = settingsService.appSettings;
</script>

<div class="decorative-split-view" class:v3={$appSettings?.featureFlags.v3}>
	<div class="left-side hide-native-scrollbar">
		<div class="left-side__content">
			{#if children}
				{@render children()}
			{/if}
		</div>
	</div>

	<div class="right-side">
		<div class="right-side-wrapper">
			{#if user}
				<div class="right-side__header">
					<div class="account-button">
						<AccountLink pop />
					</div>
				</div>
			{/if}

			{#if img}
				<div class="img-wrapper">
					{@html img}
				</div>
			{/if}

			<div class="right-side__bottom">
				{#if title || description}
					<div class="right-side__content">
						{#if title}
							<h3 class="text-18 text-bold right-side__content-title">
								{@render title()}
							</h3>
						{/if}

						{#if description}
							<p class="text-12 text-body right-side__content-description">
								{@render description()}
							</p>
						{/if}
					</div>
				{/if}

				{#if showLinks}
					{#if title || description}
						<hr class="bottom-divider" />
					{/if}

					<div class="right-side__meta">
						<div class="right-side__links">
							<button
								type="button"
								class="right-side__link"
								onclick={async () => await openExternalUrl('https://docs.gitbutler.com/')}
							>
								<Icon name="docs" opacity={0.6} />
								<span class="text-14 text-semibold">GitButler docs</span>
							</button>
							<button
								type="button"
								class="right-side__link"
								onclick={async () => await openExternalUrl('https://discord.com/invite/MmFkmaJ42D')}
							>
								<Icon name="discord" opacity={0.6} />
								<span class="text-14 text-semibold">Join community</span>
							</button>
						</div>

						<div class="wordmark">
							{@html gbLogoSvg}
						</div>
					</div>
				{/if}
			</div>
		</div>
	</div>
</div>

<style lang="postcss">
	.decorative-split-view {
		cursor: default;
		user-select: none;
		display: flex;
		flex-grow: 1;
		background-color: var(--clr-bg-1);

		&.v3 {
			height: 100%;
			border-radius: var(--radius-l);
			border: 1px solid var(--clr-border-2);
			overflow: hidden;
		}
	}

	.right-side {
		display: flex;
		flex-direction: column;
		position: relative;
	}

	.left-side {
		display: grid;
		align-items: center;
		padding: 40px 80px;
		flex: 1.3;
		background-color: var(--clr-bg-1);
		overflow-y: auto;
	}

	.left-side__content {
		display: flex;
		flex-direction: column;
		margin: 0 auto;
		width: 100%;
		max-width: 512px;
	}

	/* RIGHT SIDE */

	.right-side {
		flex: 1;
		min-width: 448px;
		background-color: var(--clr-bg-1);
		padding: 20px 20px 20px 0;
	}

	.right-side-wrapper {
		overflow: hidden;
		display: flex;
		flex-direction: column;
		align-items: center;
		height: 100%;
		background-color: var(--clr-illustration-bg);
		border-radius: 8px;
	}

	.right-side__header {
		width: 100%;
		display: flex;
		align-items: center;
		justify-content: flex-end;
		padding: 20px;
	}

	/* MIDDLE */

	.img-wrapper {
		flex: 1;
		width: 100%;
		max-width: 440px;
		overflow: hidden;
		padding: 0 24px;
	}

	.right-side__bottom {
		width: 100%;
		display: flex;
		flex-direction: column;
		padding: 32px;
	}

	.right-side__meta {
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		flex: 1;
	}

	.wordmark {
		color: var(--clr-scale-pop-30);
		right: 32px;
		bottom: 32px;
	}

	.account-button {
		right: 32px;
		top: 32px;
	}

	/* BOTTOM */

	.right-side__content {
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: 680px;
	}

	.right-side__content-title,
	.right-side__content-description {
		color: var(--clr-theme-pop-on-soft);
		text-wrap: balance;
	}

	.right-side__content-title {
		margin-bottom: 12px;
	}

	.right-side__content-description {
		opacity: 0.7;
	}

	.bottom-divider {
		--fill-color: var(--clr-theme-pop-on-soft);
		margin: 24px 0;
		width: 100%;
		height: 2px;
		border: none;
		background-image: linear-gradient(to right, var(--fill-color) 50%, transparent 50%);
		background-size: 4px 4px;
		opacity: 0.5;
	}

	.right-side__links {
		color: var(--clr-scale-pop-20);
		display: flex;
		align-items: flex-start;
		flex-direction: column;
		gap: 2px;
	}

	.right-side__link {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 6px;
		border-radius: var(--radius-m);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: oklch(from var(--clr-scale-pop-60) l c h / 0.3);
		}
	}
</style>
