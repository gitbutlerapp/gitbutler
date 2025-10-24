<script lang="ts">
	import { goto } from '$app/navigation';
	import Factoid from '$lib/components/infoFlexRow//Factoid.svelte';
	import InfoFlexRow from '$lib/components/infoFlexRow/InfoFlexRow.svelte';
	import {
		type ColumnTypes,
		type AvatarsType,
		type ChangesType
	} from '$lib/components/table/types';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import Minimap from '@gitbutler/shared/branches/Minimap.svelte';
	import { AvatarGroup, CommitStatusBadge, Icon, type CommitStatusType } from '@gitbutler/ui';

	import dayjs from 'dayjs';

	type Props = {
		columns: {
			key: keyof ColumnTypes;
			value?: ColumnTypes[keyof ColumnTypes];
			tooltip?: string;
		}[];
		href?: string;
		isTopEntry?: boolean;
		separatedTop?: boolean;
		separatedBottom?: boolean;
	};

	const userService = inject(USER_SERVICE);
	const user = userService.user;

	let { columns, href, isTopEntry = false, separatedTop, separatedBottom }: Props = $props();
	let tableMobileBreakpoint = 800;
	let isTableMobileBreakpoint = $state(window.innerWidth < tableMobileBreakpoint);

	function handleLinkClick(event: MouseEvent | KeyboardEvent) {
		if (!href) return;
		event.preventDefault();
		goto(href);
	}
</script>

<svelte:window
	on:resize={() => (isTableMobileBreakpoint = window.innerWidth < tableMobileBreakpoint)}
/>

<tr
	class="text-12 dynrow"
	role="button"
	tabIndex={0}
	onclick={handleLinkClick}
	onkeydown={(e) => e.key === 'Enter' && handleLinkClick(e)}
	class:dynrow-separatedTop={separatedTop}
	class:dynrow-separatedBottom={separatedBottom}
>
	{#if !isTableMobileBreakpoint}
		{#each columns as { key, value, tooltip }}
			<td class={[`truncate dynclmn-td dynclmn-${key}-td`]}>
				<div
					class={[
						'dynclmn',
						`dynclmn-${key}`,
						{ 'text-13 text-bold truncate': key === 'title' },
						{ 'text-12 truncate': key === 'string' },
						{ 'dynclmn-number': key === 'number' }
					]}
					title={tooltip}
				>
					{#if key === 'title'}
						<div class="truncate" title={tooltip}>
							{value}
						</div>
					{:else if key === 'number'}
						{value}
					{:else if key === 'commitGraph'}
						{@const params = columns.find((col) => col.key === 'commitGraph')
							?.value as ColumnTypes['commitGraph']}

						{#if $user}
							<Minimap
								branchUuid={params.branch.uuid}
								projectSlug={params.projectSlug}
								ownerSlug={params.ownerSlug}
								horizontal
								user={$user}
							/>
						{/if}
					{:else if key === 'avatars'}
						<AvatarGroup avatars={value as Array<AvatarsType>}></AvatarGroup>
					{:else if key === 'reviewers'}
						<div class="dynclmn-reviewers">
							{#if (value as { approvers: Array<AvatarsType> }).approvers.length > 0 || (value as { rejectors: Array<AvatarsType> }).rejectors.length > 0}
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
							{:else}
								<span class="dynclmn-placeholder">No reviews</span>
							{/if}
						</div>
					{:else if key === 'date'}
						{dayjs(value as Date).fromNow()}
					{:else if key === 'status'}
						<CommitStatusBadge
							status={value as CommitStatusType}
							kind="both"
							lineTop={!isTopEntry && !separatedTop}
							lineBottom={!separatedBottom}
						/>
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
	{:else}
		<td class="dyncell-td">
			<div class="dyncell">
				{#if columns.find((col) => col.key === 'status')}
					<div class="dyncell-status">
						<CommitStatusBadge
							status={columns.find((col) => col.key === 'status')?.value as CommitStatusType}
						/>
					</div>
				{/if}

				{#if columns.find((col) => col.key === 'title')}
					<div class="dyncell-title">
						<div class="text-13 text-bold">
							{columns.find((col) => col.key === 'title')?.value}
						</div>
					</div>
				{/if}

				<InfoFlexRow>
					{#if columns.find((col) => col.key === 'changes')}
						<Factoid label="Changes">
							<div class="dynclmn-changes">
								<span class="dynclmn-changes_additions">
									+{(columns.find((col) => col.key === 'changes')?.value as ChangesType).additions}
								</span>
								<span class="dynclmn-changes_deletions">
									-{(columns.find((col) => col.key === 'changes')?.value as ChangesType).deletions}
								</span>
							</div>
						</Factoid>
					{/if}

					{#if columns.find((col) => col.key === 'comments')}
						<Factoid label="Comments" placeholderText="No comments">
							{columns.find((col) => col.key === 'comments')?.value}
						</Factoid>
					{/if}

					{#if columns.find((col) => col.key === 'reviewers')}
						<Factoid label="Reviewers" placeholderText="No reviews">
							{@const reviewers = columns.find((col) => col.key === 'reviewers')?.value as {
								approvers: Array<AvatarsType>;
								rejectors: Array<AvatarsType>;
							}}
							{#if reviewers.approvers.length > 0 || reviewers.rejectors.length > 0}
								<div class="dynclmn-reviewers">
									<AvatarGroup
										avatars={reviewers.approvers}
										maxAvatars={2}
										icon="tick-small"
										iconColor="success"
									/>
									<AvatarGroup
										avatars={reviewers.rejectors}
										maxAvatars={2}
										icon="refresh-small"
										iconColor="warning"
									/>
								</div>
							{/if}
						</Factoid>
					{/if}

					{#if columns.find((col) => col.key === 'date')}
						<Factoid label="Date">
							{dayjs(columns.find((col) => col.key === 'date')?.value as Date).fromNow()}
						</Factoid>
					{/if}

					{#if columns.find((col) => col.key === 'number')}
						<Factoid label="Number">
							{columns.find((col) => col.key === 'number')?.value}
						</Factoid>
					{/if}

					{#if columns.find((col) => col.key === 'commitGraph')}
						<Factoid label="Commits">
							{@const params = columns.find((col) => col.key === 'commitGraph')
								?.value as ColumnTypes['commitGraph']}

							{#if $user}
								<Minimap
									branchUuid={params.branch.uuid}
									projectSlug={params.projectSlug}
									ownerSlug={params.ownerSlug}
									horizontal
									user={$user}
								/>
							{/if}
						</Factoid>
					{/if}

					{#if columns.find((col) => col.key === 'avatars')}
						<Factoid label="Authors">
							<AvatarGroup
								avatars={columns.find((col) => col.key === 'avatars')?.value as Array<AvatarsType>}
							/>
						</Factoid>
					{/if}
				</InfoFlexRow>
			</div>
		</td>
	{/if}
</tr>

<style lang="postcss">
	.dynrow {
		cursor: pointer;

		&:not(:last-child) .dyncell-td {
			border-bottom: none;
		}

		&:first-child .dyncell-td {
			border-top-right-radius: var(--radius-ml);
			border-top-left-radius: var(--radius-ml);
		}

		&:last-child .dyncell-td {
			border-bottom-right-radius: var(--radius-ml);
			border-bottom-left-radius: var(--radius-ml);
		}

		&:hover {
			.dynclmn,
			.dyncell-td {
				background-color: var(--clr-bg-1-muted);
			}
		}

		&:last-child {
			.dynclmn-td:first-child .dynclmn {
				border-bottom-left-radius: var(--radius-ml);
			}
			.dynclmn-td:last-child .dynclmn {
				border-bottom-right-radius: var(--radius-ml);
			}
		}
	}

	.dynclmn-td {
		padding: 0;

		&:last-child .dynclmn {
			border-right: 1px solid var(--clr-border-2);
		}
		&:first-child .dynclmn {
			border-left: 1px solid var(--clr-border-2);
		}
	}

	.dynclmn {
		display: flex;
		align-items: center;
		height: 58px;

		padding: 0 var(--cell-padding);
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
		color: var(--clr-text-2);
		transition: background-color var(--transition-fast);
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
		padding-left: 6px;
		color: var(--clr-theme-err-element);
		text-align: right;
	}

	/* COMMENTS CLMN */
	.dynclmn-comments {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 5px;
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
		width: 60%;
	}
	.dynclmn-title {
		display: grid;
		color: var(--clr-text-1);
	}

	.dynclmn-number {
		justify-content: flex-end;
		font-size: 12px;
		font-family: var(--font-mono);
		text-align: right;
	}

	.dynclmn-commitGraph-td {
		min-width: 120px;
		overflow: visible;
	}

	/* MOBILE CELL */
	.dyncell-td {
		padding: 0;
		border: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
		transition: background-color var(--transition-fast);
	}

	.dyncell {
		display: flex;
		flex-direction: column;
		padding: 20px;
	}

	.dyncell-status {
		margin-bottom: 12px;
	}

	.dyncell-title {
		margin-bottom: 20px;
	}

	/* MODIFIERS */
	.dynclmn-placeholder {
		opacity: 0.4;
	}

	.dynrow-separatedTop {
		.dynclmn-td {
			padding-top: 2px;

			&:first-child .dynclmn {
				border-top-left-radius: var(--radius-ml);
			}
			&:last-child .dynclmn {
				border-top-right-radius: var(--radius-ml);
			}
		}

		.dynclmn {
			border-top: 1px solid var(--clr-border-2);
		}
	}

	.dynrow-separatedBottom {
		.dynclmn-td {
			padding-bottom: 2px;

			&:first-child .dynclmn {
				border-bottom-left-radius: var(--radius-ml);
			}

			&:last-child .dynclmn {
				border-bottom-right-radius: var(--radius-ml);
			}
		}

		.dynclmn {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}
</style>
