<script lang="ts" module>
	// import type { Snippet } from 'svelte';
	export interface Props {
		title: string;
		aiMessage?: string;
		sha: string;
		upstreamSha?: string;
		date: Date;
		author?: string;
		url?: string;
		onlyContent?: boolean;
		isDone?: boolean;
		onCopy?: () => void;
		onCopyUpstream?: () => void;
		onOpen?: (url: string) => void;
		loading?: boolean;
	}
</script>

<script lang="ts">
	import CopyButton from "$components/CopyButton.svelte";
	import Icon from "$components/Icon.svelte";
	import SimpleCommitRowSkeleton from "$components/SimpleCommitRowSkeleton.svelte";
	import { getTimeAndAuthor } from "$lib/utils/getTimeAndAuthor";

	const {
		title,
		aiMessage,
		sha,
		author,
		date,
		url,
		onlyContent,
		upstreamSha,
		isDone,
		onCopy,
		onCopyUpstream,
		onOpen,
		loading,
	}: Props = $props();
</script>

{#if loading}
	<SimpleCommitRowSkeleton {onlyContent} />
{:else}
	<div class="simple-commit-item no-select" class:content-only={onlyContent}>
		{#if !onlyContent}
			{#if isDone}
				<Icon name="success" color="safe" />
			{:else}
				<Icon name="commit" />
			{/if}
		{/if}
		<div class="content">
			<span class="title text-13 text-semibold">
				{title}
			</span>

			{#if aiMessage}
				<div class="ai-message text-12 text-body">
					<p>{aiMessage}</p>
				</div>
			{/if}

			<div class="details text-11">
				<CopyButton class="details-btn" text={sha} onclick={onCopy} />

				{#if upstreamSha}
					<span class="details-divider">•</span>
					<CopyButton
						class="details-btn"
						text={upstreamSha}
						prefix="upstream"
						onclick={onCopyUpstream}
					/>
				{/if}

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
{/if}

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

		& .content {
			display: flex;
			flex: 1;
			flex-direction: column;
			overflow: hidden;
			gap: 6px;

			/* Fix because of using native dialog element */
			& span {
				text-align: left;
			}
		}

		& .title {
			overflow: hidden;
			text-overflow: ellipsis;
			white-space: nowrap;
		}

		& .ai-message {
			display: flex;
			position: relative;
			align-items: flex-end;
			max-height: 8cap;
			overflow: hidden;
			color: var(--clr-text-2);
			animation: fadeInOut 0.6s ease-in-out infinite alternate;

			&::before {
				position: absolute;
				bottom: 0;
				left: 0;
				width: 100%;
				height: 3rem;
				background: linear-gradient(to bottom, rgba(255, 210, 202, 0) 0%, var(--clr-bg-1) 100%);
				content: "";
			}
		}

		& .details {
			display: flex;
			align-items: center;
			overflow: hidden;
			gap: 4px;
			color: var(--clr-text-2);
		}

		& .details-btn {
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

		& .link-btn {
			& span {
				text-decoration: underline;
				text-underline-offset: 3px;
			}
		}

		& .details-divider {
			color: var(--clr-text-3);
			line-height: 150%;
		}
	}

	@keyframes fadeInOut {
		0% {
			opacity: 0.6;
		}
		100% {
			opacity: 1;
		}
	}
</style>
