<script lang="ts">
	import type { User } from '$lib/backend/cloud';
	import AccountLink from './AccountLink.svelte';
	import ImgThemed from './ImgThemed.svelte';

	export let user: User | undefined;
	export let imgSet: { light: string; dark: string };
</script>

<div class="decorative-split-view">
	<div class="left-side">
		<div class="left-side__content">
			<slot />
		</div>
	</div>
	<div class="right-side">
		<div class="right-side-wrapper">
			<div class="right-side__header">
				<div class="account-button">
					<AccountLink {user} pop />
				</div>
			</div>

			<div class="img-wrapper">
				<ImgThemed {imgSet} />
			</div>

			<div class="right-side__footer">
				<div class="right-side__links">
					<slot name="links" />
				</div>

				<div class="wordmark text-serif-24">GitButler</div>
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
		background-color: var(--clr-theme-container-light);
	}
	.left-side,
	.right-side {
		display: flex;
		flex-direction: column;
		position: relative;
	}

	.left-side {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--space-32) calc(var(--space-40) * 2);
		flex: 1.3;
		background-color: var(--clr-theme-container-light);
	}

	.left-side__content {
		width: 100%;
		max-width: 36rem;
		display: flex;
		flex-direction: column;
	}

	.right-side {
		flex: 1;
		min-width: 560px;
		background-color: var(--clr-theme-container-light);
		padding: var(--space-20) var(--space-20) var(--space-20) 0;
	}

	.right-side-wrapper {
		position: relative;
		overflow: hidden;
		--splitview-back-color: var(--clr-core-pop-75);
		display: flex;
		flex-direction: column;
		height: 100%;
		background-color: var(--splitview-back-color);
		border-radius: var(--space-8);
	}

	.right-side__header {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		display: flex;
		align-items: center;
		justify-content: flex-end;
		padding: var(--space-20);
	}

	.right-side__footer {
		position: absolute;
		bottom: 0;
		left: 0;
		width: 100%;
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		padding: var(--space-32);
	}

	.wordmark {
		position: absolute;
		color: var(--clr-theme-scale-pop-30);
		opacity: 0.6;
		line-height: 1;
		right: var(--space-32);
		bottom: var(--space-32);
	}

	.account-button {
		right: var(--space-32);
		top: var(--space-32);
	}

	/* links */

	.right-side__links {
		color: var(--clr-theme-scale-pop-20);
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
	}

	.img-wrapper {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		width: 100%;
		max-width: 26.25rem;
	}

	/* global */

	:global(.dark .decorative-split-view .right-side-wrapper) {
		--splitview-back-color: var(--clr-core-pop-25);
	}
</style>
