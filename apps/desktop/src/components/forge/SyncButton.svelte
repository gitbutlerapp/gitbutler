<script lang="ts">
	import { BACKEND } from "$lib/backend";
	import { lastFetched as getLastFetched } from "$lib/baseBranch/baseBranch";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { BRANCH_SERVICE } from "$lib/branches/branchService.svelte";
	import { DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button, TimeAgo, Icon, TestId } from "@gitbutler/ui";
	import { tick } from "svelte";

	interface Props {
		projectId: string;
		disabled?: boolean;
	}

	const { projectId, disabled = false }: Props = $props();

	const backend = inject(BACKEND);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const branchService = inject(BRANCH_SERVICE);
	const baseBranch = $derived(baseBranchService.baseBranch(projectId));

	const forge = inject(DEFAULT_FORGE_FACTORY);
	const listingService = $derived(forge.current.listService);

	const lastFetched = $derived(
		baseBranch.result.data ? getLastFetched(baseBranch.result.data) : undefined,
	);

	let loading = $state(false);
	let fetchProgress = $state<GitOperationProgress | undefined>();
	let fetchStartedAt = $state<number | undefined>();
	let elapsedTick = $state(Date.now());

	type GitOperationProgress = {
		operation: string;
		phase: string;
		phaseLabel: string;
		elapsedMs: number;
		detail?: string;
	};

	$effect(() => {
		if (!loading) {
			fetchProgress = undefined;
			fetchStartedAt = undefined;
			return;
		}

		const timer = window.setInterval(() => {
			elapsedTick = Date.now();
		}, 1000);
		const unlisten = backend.listen<GitOperationProgress>(
			`project://${projectId}/git_operation_progress`,
			({ payload }) => {
				if (payload.operation === "fetchFromRemotes") {
					fetchProgress = payload;
					fetchStartedAt = Date.now() - payload.elapsedMs;
				}
			},
		);

		return () => {
			window.clearInterval(timer);
			void unlisten();
		};
	});

	function formatElapsed(ms: number | undefined): string | undefined {
		if (ms === undefined) return undefined;
		const seconds = Math.floor(ms / 1000);
		if (seconds < 60) return `${seconds}s`;
		const minutes = Math.floor(seconds / 60);
		const remainingSeconds = seconds % 60;
		return `${minutes}m ${remainingSeconds}s`;
	}

	function fetchElapsedMs(): number | undefined {
		if (!fetchProgress) return fetchStartedAt === undefined ? undefined : elapsedTick - fetchStartedAt;
		if (fetchStartedAt === undefined) return fetchProgress.elapsedMs;
		return Math.max(fetchProgress.elapsedMs, elapsedTick - fetchStartedAt);
	}

	function fetchStatusLabel(): string {
		const elapsed = formatElapsed(fetchElapsedMs());
		const phase = fetchProgress?.phaseLabel ?? "Fetching";
		return elapsed ? `${phase} ${elapsed}` : phase;
	}

	function fetchTooltip(): string {
		if (!loading) return "Last fetch from upstream";
		const detail = fetchProgress?.detail;
		return detail ? `${fetchStatusLabel()}. ${detail}` : fetchStatusLabel();
	}
</script>

<Button
	testId={TestId.SyncButton}
	kind="outline"
	width="auto"
	tooltip={fetchTooltip()}
	{loading}
	{disabled}
	icon="refresh"
	reversedDirection
	onclick={async (e: MouseEvent) => {
		e.preventDefault();
		e.stopPropagation();
		loading = true;
		fetchStartedAt = Date.now();
		elapsedTick = Date.now();
		fetchProgress = {
			operation: "fetchFromRemotes",
			phase: "prepare",
			phaseLabel: "Preparing fetch",
			elapsedMs: 0,
		};
		await tick();
		try {
			await baseBranchService.fetchFromRemotes(projectId, "modal");
			await Promise.all([
				listingService?.refresh(projectId),
				baseBranch.result?.refetch(),
				branchService.refresh(),
			]);
		} finally {
			loading = false;
		}
	}}
>
	<span>
		{#if loading}
			{fetchStatusLabel()}
		{:else if lastFetched}
			<TimeAgo date={lastFetched} addSuffix={true} capitalize={true} />
		{:else}
			Refetch
		{/if}
	</span>

	{#snippet custom()}
		{#if baseBranch.response}
			<div class="target-branch">
				<Icon name="target-branch" color="var(--text-2)" />
				<span class="text-12 text-semibold">
					{baseBranch.response.remoteName}/{baseBranch.response.shortName}
				</span>
			</div>
		{/if}
	{/snippet}
</Button>

<style lang="postcss">
	.target-branch {
		display: inline-flex;
		align-items: center;
		padding-right: 2px;
		gap: 4px;
		color: var(--text-2);

		&:after {
			display: inline-block;
			width: 1px;
			height: 12px;
			margin: 0 2px 0 4px;
			background-color: var(--text-2);
			content: "";
			opacity: 0.5;
		}
	}
</style>
