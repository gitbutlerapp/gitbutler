<script lang="ts">
	import Icon from './Icon.svelte';
	import SnapshotAttachment from './SnapshotAttachment.svelte';
	import Tag from './Tag.svelte';
	import { getVSIFileIcon } from '$lib/ext-icons';

	export let entry: {
		isCurrent: boolean;
		createdAt: number;
		id: string;
		filesChanged: string[];
		title: string;
	};

	function createdOnTime(dateNumber: number) {
		const d = new Date(dateNumber);
		return d.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false });
	}
</script>

<div class="snapshot-card">
	<span class="snapshot-time text-base-12">
		{createdOnTime(entry.createdAt)}
	</span>

	<div class="snapshot-line">
		<Icon name="commit" />
	</div>

	<div class="snapshot-content">
		<div class="snapshot-details">
			{#if entry.isCurrent}
				<Tag style="pop" kind="soft">Current</Tag>
			{/if}

			<div class="snapshot-title-wrap">
				<h4 class="snapshot-title text-base-body-13 text-semibold">
					<span>{entry.title}</span>
					<span class="snapshot-sha"> • #{entry.id.slice(0, 6)}</span>
				</h4>

				<div class="restore-btn"><Tag>Resotre</Tag></div>
			</div>
		</div>

		{#if entry.filesChanged.length > 0}
			<SnapshotAttachment
				foldable={entry.filesChanged.length > 2}
				foldedAmount={entry.filesChanged.length - 2}
			>
				<div class="changed-files-list">
					{#each entry.filesChanged as filePath}
						<button
							class="snapshot-file"
							on:click={async () => {
								// Add your logic here for handling file preview
							}}
						>
							<img
								draggable="false"
								class="file-icon"
								src={getVSIFileIcon(filePath)}
								alt=""
							/>
							<div class="text-base-12 file-path-and-name">
								<span class="file-name">
									{filePath.split('/').pop()}
								</span>
								<span class="file-path">
									{filePath.replace(/\/[^/]+$/, '')}
								</span>
							</div>
						</button>
					{/each}
				</div>
			</SnapshotAttachment>
		{/if}
	</div>
</div>

<style lang="postcss">
	/* SNAPSHOT CARD */
	.snapshot-card {
		display: flex;
		gap: var(--size-10);
		padding: var(--size-10) var(--size-14) var(--size-8) var(--size-14);
		overflow: hidden;
		background-color: var(--clr-bg-1);
		/* min-height: 100px; */

		&:hover {
			background-color: var(--clr-bg-2);

			& .restore-btn {
				opacity: 1;
			}
		}
	}

	.restore-btn {
		opacity: 0;
	}

	.snapshot-time {
		color: var(--clr-text-2);
		/* background-color: #ffcf887d; */
		width: 2.15rem;

		text-align: right;
		line-height: 1.4;
		/* margin-top: var(--size-2); */
	}

	.snapshot-line {
		position: relative;
		display: flex;
		align-items: center;
		flex-direction: column;
		gap: var(--size-4);
		/* margin-top: var(--size-2); */
		/* background-color: rgba(0, 255, 255, 0.299); */

		&::after {
			position: absolute;
			top: var(--size-20);
			content: '';
			height: calc(100% - var(--size-12));
			min-height: var(--size-8);
			width: 1px;
			background-color: var(--clr-border-2);
		}
	}

	.snapshot-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--size-10);
	}

	.snapshot-details {
		display: flex;
		width: 100%;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--size-6);
	}

	.snapshot-title-wrap {
		display: flex;
		width: 100%;
	}

	.snapshot-title {
		flex: 1;
	}

	.snapshot-sha {
		white-space: nowrap;
		color: var(--clr-text-3);
	}

	/* ATTACHMENTS */

	.changed-files-list {
		display: flex;
		flex-direction: column;
		gap: var(--size-2);
		padding: var(--size-4);
	}

	.snapshot-file {
		display: flex;
		align-items: center;
		gap: var(--size-6);
		padding: var(--size-4);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}
	}

	.file-path-and-name {
		display: flex;
		gap: var(--size-6);
		overflow: hidden;
	}

	.file-path {
		color: var(--clr-text-3);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.file-name {
		color: var(--clr-text-2);
		white-space: nowrap;
	}

	.file-icon {
		width: var(--size-12);
	}
</style>
