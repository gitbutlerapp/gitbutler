<script lang="ts">
	import { type ColumnTypes, type AvatarsType, type ChangesType } from './types';
	import CommitStatusBadge, { type CommitStatusType } from '@gitbutler/ui/CommitStatusBadge.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import dayjs from 'dayjs';
	import { goto } from '$app/navigation';

	type Props = {
		columns: Array<{
			key: keyof ColumnTypes;
			value?: ColumnTypes[keyof ColumnTypes];
			tooltip?: string;
		}>;
		href?: string;
	};

	let { columns, href }: Props = $props();

	function handleLinkClick(event: MouseEvent | KeyboardEvent) {
		if (!href) return;
		event.preventDefault();
		goto(href);
	}

	console.log(columns);
</script>

<tr
	class="text-12 dynrow"
	role="button"
	tabIndex={0}
	onclick={handleLinkClick}
	onkeydown={(e) => e.key === 'Enter' && handleLinkClick(e)}
>
	{#each columns as { key, value, tooltip }}
		<td class={['truncate dynclmn dynclmn_{key}-width', { 'dynclmn-title-td': key === 'title' }]}>
			<div
				class={[
					{ 'text-13 text-bold truncate dynclmn-title': key === 'title' },
					{ 'text-12 truncate': key === 'string' }
				]}
				title={tooltip}
			>
				{#if key === 'title'}
					<div class="truncate" title={tooltip}>
						{value}
					</div>
				{:else if key === 'avatars'}
					<AvatarGroup avatars={value as Array<AvatarsType>}></AvatarGroup>
				{:else if key === 'reviewers'}
					<div class="dynclmn-reviewers">
						<AvatarGroup
							avatars={(value as { approvers: Array<AvatarsType> }).approvers}
							maxAvatars={2}
							icon="tick-small"
							iconColor="success"
						/>
						<AvatarGroup
							avatars={(value as { rejectors: Array<AvatarsType> }).rejectors}
							maxAvatars={2}
							icon="refresh-small"
							iconColor="warning"
						/>
					</div>
				{:else if key === 'date'}
					{dayjs(value as Date).fromNow()}
				{:else if key === 'status'}
					<CommitStatusBadge status={value as CommitStatusType} />
				{:else if key === 'changes'}
					<div class="dynclmn-changes">
						<span class="dynclmn-changes_additions">+{(value as ChangesType).additions}</span>
						<span class="dynclmn-changes_deletions">-{(value as ChangesType).deletions}</span>
					</div>
				{:else if key === 'comments'}
					<div class="text-12 dynclmn-comments" class:dynclmn-placeholder={!value}>
						<span>{value}</span>
						<div class="dynclmn-comments-icon"><Icon name="comments-small" /></div>
					</div>
				{:else}
					{value}
				{/if}
			</div>
		</td>
	{/each}
</tr>

<style lang="postcss">
	.dynrow {
		cursor: pointer;
		background-color: var(--clr-bg-1);

		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}

		&:last-child {
			.dynclmn:first-child {
				border-bottom-left-radius: var(--radius-ml);
			}
			.dynclmn:last-child {
				border-bottom-right-radius: var(--radius-ml);
			}
		}
	}

	.dynclmn {
		color: var(--clr-text-2);
		padding: 0 var(--cell-padding);
		height: 58px;
		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-right: 1px solid var(--clr-border-2);
		}
		&:first-child {
			border-left: 1px solid var(--clr-border-2);
		}
	}

	/* CHNAGES CLMN */
	.dynclmn-changes {
		display: flex;
		justify-content: flex-end;
	}
	.dynclmn-changes_additions {
		color: var(--clr-theme-succ-element);
		text-align: right;
	}
	.dynclmn-changes_deletions {
		color: var(--clr-theme-err-element);
		text-align: right;
		padding-left: 6px;
	}

	/* COMMENTS CLMN */
	.dynclmn-comments {
		display: flex;
		gap: 5px;
		justify-content: flex-end;
		align-items: center;
	}

	.dynclmn-comments-icon {
		display: flex;
	}

	/* TYPES */
	.dynclmn-reviewers {
		display: flex;
		gap: 10px;
	}

	.dynclmn-title-td {
		width: 100%;
	}

	.dynclmn-title {
		display: grid;
		color: var(--clr-text-1);
	}

	/* MODIFIERS */

	.dynclmn-placeholder {
		opacity: 0.4;
	}
</style>
