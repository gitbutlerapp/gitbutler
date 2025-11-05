<script lang="ts">
	import AccountLink from '$components/AccountLink.svelte';
	import { USER } from '$lib/user/user';
	import { inject } from '@gitbutler/core/context';
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
				<div class="account-button">
					<AccountLink pop />
				</div>
			{/if}

			{#if img}
				<div class="img-wrapper">
					{@html img}
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

	/* MIDDLE */

	.img-wrapper {
		flex: 1;
		width: 100%;
		max-width: 440px;
		padding: 0 24px;
		overflow: hidden;
	}

	.account-button {
		position: absolute;
		top: 32px;
		right: 32px;
	}
</style>
