<script lang="ts">
	import AccountLink from '$components/AccountLink.svelte';
	import gbLogoSvg from '$lib/assets/gb-logo.svg?raw';
	import { USER } from '$lib/user/user';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { type Snippet } from 'svelte';

	interface Props {
		hideDetails?: boolean;
		img?: string;
		testId?: string;
		children?: Snippet;
	}

	const { hideDetails, img, children, testId }: Props = $props();

	const user = inject(USER);
</script>

<div class="decorative-split-view" data-testid={testId}>
	<div class="left-side hide-native-scrollbar" data-tauri-drag-region>
		<div class="left-side__content">
			{#if children}
				{@render children()}
			{/if}
		</div>
	</div>

	<div class="right-side" data-tauri-drag-region>
		<div class="right-side-wrapper">
			{#if user && !hideDetails}
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

			{#if !hideDetails}
				<div class="right-side__bottom">
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
				</div>
			{/if}
		</div>
	</div>
</div>

<style lang="postcss">
	.decorative-split-view {
		display: flex;
		flex-grow: 1;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
		cursor: default;
	}

	.right-side {
		display: flex;
		position: relative;
		flex-direction: column;
	}

	.left-side {
		display: grid;
		flex: 1.3;
		align-items: center;
		padding: 40px 80px;
		overflow-y: auto;
		background-color: var(--clr-bg-1);
	}

	.left-side__content {
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: 512px;
		margin: 0 auto;
	}

	/* RIGHT SIDE */
	.right-side {
		flex: 1;
		min-width: 448px;
		padding: 16px;
		padding-left: 0;
		background-color: var(--clr-bg-1);
	}

	.right-side-wrapper {
		display: flex;
		flex-direction: column;
		align-items: center;
		height: 100%;
		overflow: hidden;
		border-radius: 8px;
		background-color: var(--clr-illustration-bg);
	}

	.right-side__header {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		width: 100%;
		padding: 20px;
	}

	/* MIDDLE */

	.img-wrapper {
		flex: 1;
		width: 100%;
		max-width: 440px;
		padding: 0 24px;
		overflow: hidden;
	}

	.right-side__bottom {
		display: flex;
		flex-direction: column;
		width: 100%;
		padding: 32px;
	}

	.right-side__meta {
		display: flex;
		flex: 1;
		align-items: flex-end;
		justify-content: space-between;
	}

	.wordmark {
		right: 32px;
		bottom: 32px;
		color: var(--clr-scale-pop-30);
	}

	.account-button {
		top: 32px;
		right: 32px;
	}

	/* BOTTOM */
	.right-side__links {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 2px;
		color: var(--clr-scale-pop-20);
	}

	.right-side__link {
		display: flex;
		align-items: center;
		padding: 6px;
		gap: 10px;
		border-radius: var(--radius-m);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: oklch(from var(--clr-scale-pop-60) l c h / 0.3);
		}
	}
</style>
