<script lang="ts">
	import AccountLink from '../shared/AccountLink.svelte';
	import Icon from '../shared/Icon.svelte';
	import gbLogoSvg from '$lib/assets/gb-logo.svg?raw';
	import { User } from '$lib/stores/user';
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
		background-color: var(--clr-bg-1);
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
		position: relative;
		overflow: hidden;
		display: flex;
		flex-direction: column;
		height: 100%;
		background-color: var(--clr-illustration-bg);
		border-radius: 8px;
	}

	.right-side__header {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		display: flex;
		align-items: center;
		justify-content: flex-end;
		padding: 20px;
	}

	.right-side__footer {
		position: absolute;
		bottom: 0;
		left: 0;
		width: 100%;
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		padding: 32px;
	}

	.wordmark {
		position: absolute;
		color: var(--clr-scale-pop-30);
		right: 32px;
		bottom: 32px;
	}

	.account-button {
		right: 32px;
		top: 32px;
	}

	/* links */

	.right-side__links {
		color: var(--clr-scale-pop-20);
		display: flex;
		align-items: flex-start;
		flex-direction: column;
		gap: 4px;
	}

	.right-side__link {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 6px;
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
		max-width: 400px;
		overflow: hidden;
	}
</style>
