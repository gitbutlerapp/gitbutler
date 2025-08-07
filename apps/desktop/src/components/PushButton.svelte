<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { writeClipboard } from '$lib/backend/clipboard';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import {
		branchHasConflicts,
		branchHasUnpushedCommits,
		branchRequiresForcePush
	} from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { getBranchNameFromRef } from '$lib/utils/branch';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import {
		Button,
		Checkbox,
		Modal,
		TestId,
		SimpleCommitRow,
		ScrollableContainer
	} from '@gitbutler/ui';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
		multipleBranches: boolean;
		isLastBranchInStack?: boolean;
		isFirstBranchInStack?: boolean;
	};

	const {
		projectId,
		branchName,
		stackId,
		multipleBranches,
		isFirstBranchInStack,
		isLastBranchInStack
	}: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const projectsService = inject(PROJECTS_SERVICE);
	const uiState = inject(UI_STATE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const branchDetails = $derived(stackService.branchDetails(projectId, stackId, branchName));
	const projectResult = $derived(projectsService.getProject(projectId));
	const [pushStack, pushResult] = stackService.pushStack;

	// Store current context for modals
	let currentRequiresForce = $state(false);
	let _currentProject = $state<any>(undefined);

	// TODO: REPLACE WITH REAL DATA
	const potentialOverwrittenCommits = [
		{
			id: 'abc123f45',
			descriptionTitle: 'Fix user authentication bug',
			author: { name: 'John Doe' },
			createdAt: new Date('2025-01-05T10:30:00Z')
		},
		{
			id: 'def456a78',
			descriptionTitle: 'Update README documentation',
			author: { name: 'Jane Smith' },
			createdAt: new Date('2025-01-04T14:20:00Z')
		},
		{
			id: 'ghi789b12',
			descriptionTitle: 'Add new feature for file upload',
			author: { name: 'Bob Johnson' },
			createdAt: new Date('2025-01-04T09:15:00Z')
		},
		{
			id: 'jkl012c34',
			descriptionTitle: 'Refactor database queries',
			author: { name: 'Alice Brown' },
			createdAt: new Date('2025-01-03T16:45:00Z')
		},
		{
			id: 'mno345d56',
			descriptionTitle: 'Fix CSS styling issues',
			author: { name: 'Charlie Wilson' },
			createdAt: new Date('2025-01-03T11:30:00Z')
		},
		{
			id: 'pqr678e90',
			descriptionTitle: 'Update dependencies',
			author: { name: 'David Lee' },
			createdAt: new Date('2025-01-02T13:20:00Z')
		},
		{
			id: 'stu901f23',
			descriptionTitle: 'Add unit tests for core functions',
			author: { name: 'Eva Martinez' },
			createdAt: new Date('2025-01-02T08:45:00Z')
		},
		{
			id: 'vwx234g45',
			descriptionTitle: 'Optimize image loading performance',
			author: { name: 'Frank Chen' },
			createdAt: new Date('2025-01-01T15:10:00Z')
		},
		{
			id: 'yz567h78',
			descriptionTitle: 'Fix memory leak in worker threads',
			author: { name: 'Grace Taylor' },
			createdAt: new Date('2024-12-31T12:00:00Z')
		},
		{
			id: 'abc890i01',
			descriptionTitle: 'Implement dark mode theme',
			author: { name: 'Henry Davis' },
			createdAt: new Date('2024-12-30T17:30:00Z')
		},
		{
			id: 'def123j23',
			descriptionTitle: 'Add error logging system',
			author: { name: 'Iris Kim' },
			createdAt: new Date('2024-12-30T09:45:00Z')
		},
		{
			id: 'ghi456k45',
			descriptionTitle: 'Update API endpoints',
			author: { name: 'Jack Robinson' },
			createdAt: new Date('2024-12-29T14:15:00Z')
		},
		{
			id: 'jkl789l67',
			descriptionTitle: 'Fix mobile responsiveness',
			author: { name: 'Kate Anderson' },
			createdAt: new Date('2024-12-29T10:30:00Z')
		},
		{
			id: 'mno012m89',
			descriptionTitle: 'Add data validation',
			author: { name: 'Luke Thompson' },
			createdAt: new Date('2024-12-28T16:20:00Z')
		}
	];

	function handleClick(requiresForce: boolean, project?: any) {
		currentRequiresForce = requiresForce;
		_currentProject = project;

		// Check for force push protection first if this is a force push
		if (requiresForce && project?.force_push_protection) {
			forcePushProtectionModal?.show();
			return;
		}

		if (multipleBranches && !isLastBranchInStack && !$doNotShowPushBelowWarning) {
			confirmationModal?.show();
			return;
		}

		push(requiresForce);
	}

	async function push(requiresForce: boolean) {
		const pushResult = await pushStack({
			projectId,
			stackId,
			withForce: requiresForce,
			branch: branchName
		});

		const upstreamBranchNames = pushResult.branchToRemote
			.map(([_, refname]) => getBranchNameFromRef(refname, pushResult.remote))
			.filter(isDefined);
		if (upstreamBranchNames.length === 0) return;
		uiState.project(projectId).branchesToPoll.add(...upstreamBranchNames);
	}

	const loading = $derived(pushResult.current.isLoading);

	function getButtonTooltip(hasThingsToPush: boolean, hasConflicts: boolean): string | undefined {
		if (!hasThingsToPush) {
			return 'No commits to push';
		}
		if (hasConflicts) {
			return 'In order to push, please resolve any conflicted commits.';
		}
		if (multipleBranches && !isLastBranchInStack) {
			return 'Push this and all branches below';
		}

		return undefined;
	}

	const doNotShowPushBelowWarning = persisted<boolean>(false, 'doNotShowPushBelowWarning');
	let confirmationModal = $state<ReturnType<typeof Modal>>();
	let forcePushProtectionModal = $state<ReturnType<typeof Modal>>();
</script>

<ReduxResult {projectId} result={branchDetails.current}>
	{#snippet children(branchDetails)}
		{@const requiresForce = branchRequiresForcePush(branchDetails)}
		{@const hasThingsToPush = branchHasUnpushedCommits(branchDetails)}
		{@const hasConflicts = branchHasConflicts(branchDetails)}

		<ReduxResult {projectId} result={projectResult.current}>
			{#snippet children(project)}
				<Button
					testId={TestId.StackPushButton}
					kind={isFirstBranchInStack ? 'solid' : 'outline'}
					size="tag"
					style="neutral"
					{loading}
					disabled={!hasThingsToPush || hasConflicts}
					tooltip={getButtonTooltip(hasThingsToPush, hasConflicts)}
					onclick={() => handleClick(requiresForce, project)}
					icon={multipleBranches && !isLastBranchInStack ? 'push-below' : 'push'}
				>
					{requiresForce ? 'Force push' : 'Push'}
				</Button>
			{/snippet}
		</ReduxResult>

		<Modal
			title="Push with dependencies"
			width="small"
			bind:this={confirmationModal}
			onSubmit={async (close) => {
				close();
				push(currentRequiresForce);
			}}
		>
			<p>
				You're about to push <span class="text-bold">{branchName}</span>. To maintain the correct
				history, GitButler will also push all branches below this branch in the stack.
			</p>

			{#snippet controls(close)}
				<div class="modal-footer">
					<div class="flex flex-1">
						<label for="dont-show-again" class="modal-footer__checkbox">
							<Checkbox name="dont-show-again" small bind:checked={$doNotShowPushBelowWarning} />
							<span class="text-12"> Don’t show again</span>
						</label>
					</div>
					<Button
						kind="outline"
						onclick={() => {
							$doNotShowPushBelowWarning = false;
							close();
						}}>Cancel</Button
					>
					<Button testId={TestId.StackConfirmPushModalButton} style="pop" type="submit" width={90}
						>Push</Button
					>
				</div>
			{/snippet}
		</Modal>

		<Modal
			title="Protected force push"
			width={480}
			type="warning"
			bind:this={forcePushProtectionModal}
			onSubmit={async (close) => {
				close();
				push(currentRequiresForce);
			}}
		>
			<p class="description">
				Your force push was blocked because the remote branch contains <span
					class="text-bold text-nowrap">{potentialOverwrittenCommits.length} commits</span
				>
				your local branch doesn’t include. To prevent overwriting history,
				<span class="text-bold">cancel and pull & integrate</span> the changes.
			</p>
			<div class="scroll-wrap">
				<ScrollableContainer maxHeight="14rem">
					{#each potentialOverwrittenCommits as commit}
						{@const commitUrl = forge.current.commitUrl(commit.id)}
						<SimpleCommitRow
							title={commit.descriptionTitle ?? ''}
							sha={commit.id}
							date={commit.createdAt}
							author={commit.author.name}
							url={commitUrl}
							onOpen={(url) => openExternalUrl(url)}
							onCopy={() => writeClipboard(commit.id)}
						/>
					{/each}
				</ScrollableContainer>
			</div>

			{#snippet controls(close)}
				<Button style="pop" onclick={close}>Cancel</Button>
				<Button kind="outline" type="submit">Force push anyway</Button>
			{/snippet}
		</Modal>
	{/snippet}
</ReduxResult>

<style>
	/* MODAL */
	.modal-footer {
		display: flex;
		width: 100%;
		gap: 6px;
	}

	.modal-footer__checkbox {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	/* COMMITS SCROLL CONTAINER */
	.description {
		margin: 0 0 16px;
	}
	.scroll-wrap {
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}
</style>
