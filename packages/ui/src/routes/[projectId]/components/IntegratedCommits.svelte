<script lang="ts">
	import Link from '$lib/components/Link.svelte';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import { flip } from 'svelte/animate';
	import type { CrossfadeParams, TransitionConfig } from 'svelte/transition';
	import CommitCard from './CommitCard.svelte';

	export let branch: Branch;
	export let base: BaseBranch | undefined | null;
	export let projectId: string;

	export let receive: (
		node: any,
		params: CrossfadeParams & {
			key: any;
		}
	) => () => TransitionConfig;

	export let send: (
		node: any,
		params: CrossfadeParams & {
			key: any;
		}
	) => () => TransitionConfig;

	$: integratedCommits = branch.commits.filter((c) => c.isIntegrated);

	function baseUrl(target: BaseBranch | undefined | null) {
		if (!target) return undefined;
		const parts = target.branchName.split('/');
		return `${target.repoBaseUrl}/commits/${parts[parts.length - 1]}`;
	}
</script>

{#if integratedCommits.length > 0}
	<div class="relative">
		<div
			class="dark:form-dark-600 absolute top-4 ml-[0.75rem] w-px bg-gradient-to-b from-light-600 via-light-600 via-90% dark:from-dark-400 dark:via-dark-400"
			style="height: calc(100% - 1rem);"
		/>

		<div class="relative flex flex-grow flex-col gap-2">
			<div
				class="dark:form-dark-600 absolute top-4 ml-[0.75rem] h-px w-6 bg-gradient-to-r from-light-600 via-light-600 via-10% dark:from-dark-400 dark:via-dark-400"
			/>

			<div class="relative max-w-full flex-grow overflow-hidden py-2 pl-12 pr-2 font-mono text-sm">
				<Link
					target="_blank"
					rel="noreferrer"
					href={baseUrl(base)}
					class="inline-block max-w-full truncate text-sm font-bold"
				>
					integrated to {base?.branchName}
				</Link>
			</div>

			{#each integratedCommits as commit (commit.id)}
				<div
					class="flex w-full items-center gap-x-2 pb-2 pr-4"
					in:receive={{ key: commit.id }}
					out:send={{ key: commit.id }}
					animate:flip
				>
					<div class="ml-[0.4rem] mr-1.5">
						<div
							class="h-3 w-3 rounded-full border-2 border-light-600 bg-light-600 dark:border-dark-400 dark:bg-dark-400"
							class:bg-light-500={commit.isRemote}
							class:dark:bg-dark-500={commit.isRemote}
						/>
					</div>
					<CommitCard {commit} {projectId} commitUrl={base?.commitUrl(commit.id)} />
				</div>
			{/each}
		</div>
	</div>
{/if}
