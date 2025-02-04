<script lang="ts">
	import EmptyStack from '$lib/assets/illustrations/empty-stack-placeholder.svg?raw';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
</script>

<div class="placeholder">
	<div class="placeholder__top">
		<div class="placeholder__top--svg">
			{@html EmptyStack}
		</div>
		<div class="placeholder-text">
			<div class="text-18 text-semibold text-clr1">This is a new stack</div>
			<div class="text-13 text-clr2">
				<!-- Weird string rendering required because prettier seems to break this -->
				{`Stack is a workflow for building branches sequentially to break features into smaller parts. You can also choose a regular single-branch flow.`}
			</div>
		</div>
	</div>
	<div class="placeholder__bottom">
		<div class="placeholder__bottom--left">
			<div class="text-16 text-semibold">Tips</div>
			<Link
				href="https://docs.gitbutler.com/features/stacked-branches"
				target="_blank"
				underline={false}
				class="text-13 text-semibold text-clr3">What is a stack?</Link
			>
			<Link
				href="https://docs.gitbutler.com/guide"
				target="_blank"
				underline={false}
				class="text-13 text-semibold text-clr3">Commit and push</Link
			>
			<Link
				href="https://cloud.gitbutler.com"
				target="_blank"
				underline={false}
				class="text-13 text-semibold text-clr3">Manage commits</Link
			>
		</div>
		<div class="placeholder__bottom--right">
			<Link
				href="https://docs.gitbutler.com"
				target="_blank"
				underline={false}
				externalIcon={false}
				class="text-13 text-semibold text-clr2"><Icon name="doc" /> GitButler Docs</Link
			>
			<Link
				href="https://github.com/gitbutlerapp/gitbutler"
				target="_blank"
				underline={false}
				externalIcon={false}
				class="text-13 text-semibold text-clr2"><Icon name="github" /> Source Code</Link
			>
			<Link
				href="https://discord.com/invite/MmFkmaJ42D"
				target="_blank"
				underline={false}
				externalIcon={false}
				class="text-13 text-semibold text-clr2"><Icon name="discord" /> Join Community</Link
			>
		</div>
	</div>
</div>

<style>
	.placeholder {
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		justify-content: flex-start;
		overflow: hidden;
	}

	.placeholder__top {
		flex: 1;
		display: flex;
		flex-direction: column;

		gap: 24px;
		padding: 48px;
	}

	.placeholder__top--svg {
		height: 200px;
		width: 300px;
	}

	.placeholder__bottom {
		width: 100%;
		display: flex;
		padding: 48px;

		border-top: 1px solid var(--clr-border-2);
	}

	.placeholder__bottom--left > div:first-child {
		margin-bottom: 20px;
	}

	.placeholder__bottom--right,
	.placeholder__bottom--left {
		flex: 0.5;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.placeholder__bottom--right {
		justify-content: end;
	}

	.placeholder-text {
		text-align: left;
		line-height: 1 !important;
		display: inline;

		white-space: pre-wrap;
		text-wrap: wrap;
	}
</style>
