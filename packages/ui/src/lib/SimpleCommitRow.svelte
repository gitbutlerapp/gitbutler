<script lang="ts" module>
	// import type { Snippet } from 'svelte';
	export interface Props {
		title: string;
		sha: string;
		author?: string;
		date: Date;
		onCopy?: () => void;
		onUrlOpen?: () => void;
	}
</script>

<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import { getTimeAndAuthor } from '$lib/utils/getTimeAndAuthor';

	let { title, sha, author, date, onCopy, onUrlOpen }: Props = $props();
</script>

<div class="simple-commit-item no-select">
	<Icon name="commit" />
	<div class="content">
		<span class="title text-13 text-semibold">
			{title}
		</span>
		<div class="details text-11">
			<button class="details-btn" onclick={onCopy}>
				<span>{sha}</span>
				<Icon name="copy-small" />
			</button>
			<span class="details-divider">•</span>
			<button class="details-btn" onclick={onUrlOpen}>
				<span>Open</span>
				<Icon name="open-link" />
			</button>

			<span class="details-divider">•</span>
			<span class="truncate">{getTimeAndAuthor(date, author)}</span>
		</div>
	</div>
</div>

<style lang="postcss">
	.simple-commit-item {
		display: flex;
		gap: 10px;
		padding: 12px 14px 14px 12px;
		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-bottom: none;
		}

		.content {
			display: flex;
			flex-direction: column;
			gap: 6px;
			overflow: hidden;
		}

		.title {
			overflow: hidden;
			text-overflow: ellipsis;
			white-space: nowrap;
		}

		.details {
			display: flex;
			align-items: center;
			gap: 4px;
			color: var(--clr-text-2);
			overflow: hidden;
		}

		.details-copy-btn {
			text-decoration: underline;
			text-decoration-style: dotted;
			text-underline-offset: 3px;
		}

		.details-divider {
			color: var(--clr-text-3);
			line-height: 150%;
		}
	}
</style>
