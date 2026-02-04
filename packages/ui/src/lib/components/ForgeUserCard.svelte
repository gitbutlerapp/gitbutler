<script lang="ts">
	import Button from '$components/Button.svelte';
	import Avatar from '$components/avatar/Avatar.svelte';
	import { CardGroup } from '$components/cardGroup';
	import type { Snippet } from 'svelte';

	type Props = {
		username: string;
		avatarUrl: string | null;
		email?: string | null;
		isError: boolean;
		isLoading: boolean;
		badge: Snippet;
		onForget: () => void;
		isForgetLoading: boolean;
	};

	const {
		username,
		avatarUrl,
		email,
		isError,
		isLoading,
		badge,
		onForget,
		isForgetLoading
	}: Props = $props();
</script>

<CardGroup.Item>
	{#snippet iconSide()}
		<div class="avatar">
			{#if isError}
				<svg
					class="icon"
					width="16"
					height="16"
					viewBox="0 0 16 16"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
				>
					<g clip-path="url(#clip0_15331_2509)">
						<path
							d="M10.1436 0.5C10.7873 0.5 11.4044 0.759893 11.8545 1.2207L14.8154 4.25293C15.2542 4.70211 15.5 5.30627 15.5 5.93457V10.0654C15.5 10.6937 15.2542 11.2979 14.8154 11.7471L11.8545 14.7793C11.4044 15.2401 10.7873 15.5 10.1436 15.5H5.78906C5.15341 15.5 4.54358 15.2466 4.09473 14.7959L1.2002 11.8896C0.751475 11.4391 0.5 10.8277 0.5 10.1914V5.92676C0.5 5.30429 0.741324 4.70586 1.17285 4.25781L4.08594 1.23242C4.53683 0.764508 5.15823 0.5 5.80762 0.5H10.1436ZM7.97168 9.92676C7.72683 9.92682 7.52467 10.1262 7.52441 10.377C7.52441 10.6279 7.72668 10.8281 7.97168 10.8281C8.21672 10.8281 8.41895 10.6279 8.41895 10.377C8.41869 10.1262 8.21657 9.92676 7.97168 9.92676ZM7.97168 5.17188C7.72668 5.17193 7.52441 5.37211 7.52441 5.62305V7.52441C7.52441 7.77535 7.72668 7.97553 7.97168 7.97559C8.21673 7.97559 8.41895 7.77538 8.41895 7.52441V5.62305C8.41895 5.37208 8.21673 5.17188 7.97168 5.17188Z"
							fill="var(--clr-theme-danger-element)"
							stroke="var(--clr-bg-1)"
							stroke-width="1.2"
						/>
					</g>
				</svg>
			{/if}

			<Avatar size="large" {username} srcUrl={avatarUrl} />
		</div>
	{/snippet}

	{#snippet title()}
		{username}
		<span class="m-l-4">
			{@render badge?.()}
		</span>
	{/snippet}
	{#snippet caption()}
		{#if email}
			{email}
		{:else if isError}
			Error loading user info
		{:else if isLoading}
			Loading...
		{:else}
			No email available
		{/if}
	{/snippet}

	{#snippet actions()}
		<Button kind="outline" icon="bin-small" onclick={onForget} loading={isForgetLoading}
			>Forget</Button
		>
	{/snippet}
</CardGroup.Item>

<style>
	.avatar {
		position: relative;
		align-self: flex-start;
		height: fit-content;
	}

	.icon {
		display: flex;
		z-index: 1;
		position: absolute;
		right: -4px;
		bottom: -4px;
		align-items: center;
		justify-content: center;
	}
</style>
