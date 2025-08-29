<script lang="ts">
	import { goto } from '$app/navigation';
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import { eventTimeStamp } from '@gitbutler/shared/branches/utils';
	import { inject } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { APP_STATE } from '@gitbutler/shared/redux/store.svelte';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { getRulesList } from '@gitbutler/shared/rules/rulesPreview.svelte';
	import { RULES_SERVICE } from '@gitbutler/shared/rules/rulesService';
	import type { Rule } from '@gitbutler/shared/rules/types';

	// Get authentication service and check if user is logged in
	const authService = inject(AUTH_SERVICE);
	const routes = inject(WEB_ROUTES_SERVICE);

	// If there is no token (user not logged in), redirect to home
	$effect(() => {
		if (!authService.token.current) {
			goto(routes.homePath());
		}
	});

	const rulesService = inject(RULES_SERVICE);
	const appState = inject(APP_STATE);

	const rulesList = $derived(getRulesList(appState, rulesService));

	function getTimeStamp(rule: Rule): string {
		return eventTimeStamp(rule);
	}
</script>

<svelte:head>
	<title>Rules</title>
</svelte:head>

<Loading loadable={rulesList.current}>
	{#snippet children(rulesList)}
		{#if rulesList.length > 0}
			<table class="rules-table">
				<thead class="rules-table__head">
					<tr>
						<th>
							<div class="text-12 rule-title">Title</div>
						</th>
						<th>
							<div class="text-12 rule-title">Project</div>
						</th>
						<th>
							<div class="text-12 rule-title">Created At</div>
						</th>
					</tr>
				</thead>
				<tbody>
					{#each rulesList as rule}
						<tr class="rules-table__row">
							<td
								><div class="text-13 truncate">
									{rule.title}
								</div>
							</td>
							<td><div class="text-13 truncate">{rule.projectSlug}</div></td>
							<td
								><div class="text-13" title={new Date(rule.createdAt).toLocaleString()}>
									{getTimeStamp(rule)}
								</div></td
							>
						</tr>
					{/each}
				</tbody>
			</table>
		{:else}
			<p>No rules found.</p>
		{/if}
	{/snippet}
</Loading>

<style lang="postcss">
	.rules-table {
		--cell-padding: 14px;

		width: 100%;
		border-collapse: collapse;
		border-spacing: 0;
	}

	.rules-table__head th {
		border: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
		vertical-align: top;

		&:not(:last-child) {
			border-right: none;
		}

		&:not(:first-child) {
			border-left: none;
		}

		&:first-child {
			border-top-left-radius: var(--radius-ml);
		}

		&:last-child {
			border-top-right-radius: var(--radius-ml);
		}
	}

	.rule-title {
		padding: var(--cell-padding);
		color: var(--clr-text-2);
		text-align: left;
	}

	.rules-table__row {
		border: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
		cursor: pointer;
		transition: background-color 0.2s ease;

		&:hover {
			background-color: var(--clr-bg-3);
		}

		td {
			padding: var(--cell-padding);
			color: var(--clr-text-1);
			text-align: left;

			&:not(:last-child) {
				border-right: 1px solid var(--clr-border-2);
			}
		}
	}
</style>
