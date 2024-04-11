<script lang="ts">
	import AccountLink from './AccountLink.svelte';
	import Icon from './Icon.svelte';
	import gbLogoSvg from '$lib/assets/gb-logo.svg?raw';
	import { User } from '$lib/backend/httpClient';
	import { getContextStore } from '$lib/utils/context';

	export let showLinks: boolean = true;
	export let img: string | undefined = undefined;

	const user = getContextStore(User);
</script>

<div class="decorative-split-view">
	<div class="left-side hide-native-scrollbar" data-tauri-drag-region>
		<div class="left-side__content">
			<slot />
		</div>
	</div>

	<div class="right-side">
		<div class="right-side-wrapper" data-tauri-drag-region>
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

			{#if showLinks}
				<div class="right-side__footer">
					<div class="right-side__links">
						<a
							class="right-side__link"
							target="_blank"
							href="https://docs.gitbutler.com/features/virtual-branches/branch-lanes"
						>
							<Icon name="docs" opacity={0.6} />
							<span class="text-base-14 text-semibold">GitButler docs</span>
						</a>
						<a
							class="right-side__link"
							target="_blank"
							href="https://discord.com/invite/MmFkmaJ42D"
						>
							<Icon name="discord" opacity={0.6} />
							<span class="text-base-14 text-semibold">Join community</span>
						</a>
					</div>

					<div class="wordmark">
						{@html gbLogoSvg}
					</div>
				</div>
			{/if}
		</div>
	</div>
</div>

<style lang="postcss">
	.decorative-split-view {
		cursor: default;
		user-select: none;
		display: flex;
		flex-grow: 1;
		background-color: var(--clr-bg-main);
	}

	.right-side {
		display: flex;
		flex-direction: column;
		position: relative;
	}

	.left-side {
		display: grid;
		align-items: center;
		padding: var(--size-40) calc(var(--size-40) * 2);
		flex: 1.3;
		background-color: var(--clr-bg-main);
		overflow-y: auto;
	}

	.left-side__content {
		display: flex;
		flex-direction: column;
		margin: 0 auto;
		width: 100%;
		max-width: 32rem;
	}

	/* RIGHT SIDE */

	.right-side {
		flex: 1;
		min-width: 28rem;
		background-color: var(--clr-bg-main);
		padding: var(--size-20) var(--size-20) var(--size-20) 0;
	}

	.right-side-wrapper {
		position: relative;
		overflow: hidden;
		display: flex;
		flex-direction: column;
		height: 100%;
		background-color: var(--clr-illustration-bg);
		border-radius: var(--size-8);
	}

	.right-side__header {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		display: flex;
		align-items: center;
		justify-content: flex-end;
		padding: var(--size-20);
	}

	.right-side__footer {
		position: absolute;
		bottom: 0;
		left: 0;
		width: 100%;
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		padding: var(--size-32);
	}

	.wordmark {
		position: absolute;
		color: var(--clr-scale-pop-30);
		right: var(--size-32);
		bottom: var(--size-32);
	}

	.account-button {
		right: var(--size-32);
		top: var(--size-32);
	}

	/* links */

	.right-side__links {
		color: var(--clr-scale-pop-20);
		display: flex;
		align-items: flex-start;
		flex-direction: column;
		gap: var(--size-4);
	}

	.right-side__link {
		display: flex;
		align-items: center;
		gap: var(--size-10);
		padding: var(--size-6);
		border-radius: var(--radius-m);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: oklch(from var(--clr-scale-pop-50) l c h / 0.15);
		}
	}

	.img-wrapper {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		width: 100%;
		max-width: 25rem;
		overflow: hidden;
	}
</style>
