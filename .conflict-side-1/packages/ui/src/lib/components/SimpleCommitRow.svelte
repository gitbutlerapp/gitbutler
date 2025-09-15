<script lang="ts" module>
	// import type { Snippet } from 'svelte';
	export interface Props {
		title: string;
		sha: string;
		date: Date;
		author?: string;
		url?: string;
		onlyContent?: boolean;
		onCopy?: () => void;
		onOpen?: (url: string) => void;
	}
</script>

<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import { getTimeAndAuthor } from '$lib/utils/getTimeAndAuthor';

	const { title, sha, author, date, url, onlyContent, onCopy, onOpen }: Props = $props();
</script>

<div class="simple-commit-item no-select" class:content-only={onlyContent}>
	{#if !onlyContent}
		<Icon name="commit" />
	{/if}
	<div class="content">
		<span class="title text-13 text-semibold">
			{title}
		</span>
		<div class="details text-11">
			<button type="button" class="details-btn copy-btn" onclick={onCopy}>
				<span>{sha.substring(0, 7)}</span>
				<Icon name="copy-small" />
			</button>

			{#if url && onOpen}
				<span class="details-divider">•</span>
				<button type="button" class="details-btn link-btn" onclick={() => onOpen(url)}>
					<span>Open</span>
					<Icon name="open-link" />
				</button>
			{/if}

			<span class="details-divider">•</span>
			<span class="truncate">{getTimeAndAuthor(date, author)}</span>
		</div>
	</div>
</div>

<style lang="postcss">
	.simple-commit-item {
		display: flex;
		overflow: hidden;
		gap: 10px;

		&:not(.content-only) {
			padding: 12px 14px 14px 12px;
			border-bottom: 1px solid var(--clr-border-2);

			&:last-child {
				border-bottom: none;
			}
		}

		.content {
			display: flex;
			flex-direction: column;
			overflow: hidden;
			gap: 6px;

			/* Fix because of using native dialog element */
			& span {
				text-align: left;
			}
		}

		.title {
			overflow: hidden;
			text-overflow: ellipsis;
			white-space: nowrap;
		}

		.details {
			display: flex;
			align-items: center;
			overflow: hidden;
			gap: 4px;
			color: var(--clr-text-2);
		}

		.details-btn {
			display: flex;
			align-items: center;
			transition: color var(--transition-fast);

			& span {
				margin-right: 4px;
			}

			&:hover {
				color: var(--clr-text-1);
			}
		}

		.copy-btn {
			& span {
				text-decoration: underline;
				text-decoration-style: dotted;
				text-underline-offset: 3px;
			}
		}

		.link-btn {
			& span {
				text-decoration: underline;
				text-underline-offset: 3px;
			}
		}

		.details-divider {
			color: var(--clr-text-3);
			line-height: 150%;
		}
	}
</style>
