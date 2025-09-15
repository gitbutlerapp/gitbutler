<script lang="ts">
	import ProjectNameLabel from '$components/ProjectNameLabel.svelte';
	import { OnboardingEvent, POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import { BACKEND } from '$lib/backend';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { unique } from '$lib/utils/array';
	import { getBestBranch, getBestRemote, getBranchRemoteFromRef } from '$lib/utils/branch';
	import { inject } from '@gitbutler/core/context';
	import { Button, Select, SelectItem, TestId, Link, Icon } from '@gitbutler/ui';
	import type { RemoteBranchInfo } from '$lib/baseBranch/baseBranch';

	interface Props {
		projectId: string;
		projectName: string;
		remoteBranches: RemoteBranchInfo[];
		onBranchSelected?: (branch: string[]) => void;
	}

	const { projectId, projectName, remoteBranches, onBranchSelected }: Props = $props();

	const backend = inject(BACKEND);
	const posthog = inject(POSTHOG_WRAPPER);

	let loading = $state<boolean>(false);
	let showMoreInfo = $state<boolean>(false);

	// split all the branches by the first '/' and gather the unique remote names
	// then turn remotes into an array of objects with a 'name' and 'value' key
	const remotes = $derived(
		unique(remoteBranches.map((b) => getBranchRemoteFromRef(b.name))).filter(
			(r): r is string => !!r
		)
	);

	let selectedBranch = $state<RemoteBranchInfo | undefined>(undefined);
	const defaultBranch = $derived(getBestBranch(remoteBranches.slice()));
	const branch = $derived(selectedBranch ?? defaultBranch);

	let selectedRemote = $state<string | undefined>(undefined);
	const defaultRemote = $derived(
		(branch && getBranchRemoteFromRef(branch.name)) ?? getBestRemote(remotes)
	);
	const remote = $derived(selectedRemote ?? defaultRemote);

	async function onSetTargetClick() {
		if (!branch || !remote) return;
		posthog.captureOnboarding(OnboardingEvent.ProjectSetupContinue);
		onBranchSelected?.([branch.name, remote]);
	}

	const projectsService = inject(PROJECTS_SERVICE);
	async function deleteProjectAndGoBack() {
		await projectsService.deleteProject(projectId);
	}
</script>

<div class="project-setup">
	<div class="project-setup__info">
		<ProjectNameLabel {projectName} />
		<h3 class="text-14 text-body text-bold">Target branch</h3>
		<p class="text-12 text-body">
			This is the branch that you consider "production", normally something like "origin/master" or
			"upstream/main".
			<Link href="https://docs.gitbutler.com/overview#target-branch">Read more</Link>
		</p>
	</div>

	<div class="project-setup__fields">
		<div class="project-setup__field-wrap" data-testid={TestId.ProjectSetupPageTargetBranchSelect}>
			<Select
				value={branch?.name}
				options={remoteBranches.map((b) => ({ label: b.name, value: b.name }))}
				wide
				onselect={(value) => {
					selectedBranch = { name: value };
				}}
				label="Current target branch"
				searchable
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={item.value === branch?.name} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		</div>

		{#if remotes.length > 1}
			<div class="project-setup__field-wrap">
				<Select
					value={remote}
					options={remotes.map((r) => ({ label: r, value: r }))}
					onselect={(value) => {
						const newSelectedRemote = remotes.find((r) => r === value);
						selectedRemote = newSelectedRemote ?? remote;
					}}
				>
					{#snippet itemSnippet({ item, highlighted })}
						<SelectItem selected={item.value === remote} {highlighted}>
							{item.label}
						</SelectItem>
					{/snippet}
				</Select>

				<p class="project-setup__description-text text-12 text-body">
					You have branches from multiple remotes. If you want to specify a remote for creating
					branches that is different from the remote that your target branch is on, change it here.
				</p>
			</div>
		{/if}
	</div>

	<div class="project-setup__info">
		<h3 class="text-14 text-body text-bold">Workspace</h3>
		<p class="text-12 text-body">
			Pressing <b>Continue</b> will take your repository to the <b>gitbutler/workspace</b> branch
		</p>

		<div
			class="project-setup__what-the-faq"
			role="presentation"
			onclick={() => (showMoreInfo = !showMoreInfo)}
		>
			<div class="project-setup__fold-icon" class:rotate-icon={showMoreInfo}>
				<Icon name="chevron-right" />
			</div>
			<h3 class="text-13 text-body text-bold">Sure, but why, though?</h3>
		</div>
		{#if showMoreInfo}
			<div class="project-setup__what-the-faq-details">
				<p class="text-12 text-body">
					In order to support working on multiple branches simultaneously, GitButler creates and
					automatically manages a special branch <b>gitbutler/workspace</b>. You can always switch
					back and forth as needed between normal git branches and the Gitbutler workspace.
					<Link href="https://docs.gitbutler.com/features/virtual-branches/integration-branch"
						>Read more</Link
					>
				</p>
			</div>
		{/if}
	</div>

	<!-- <div class="card features-wrapper">
		<SetupFeature labelFor="aiGenEnabled">
			{#snippet icon()}
				<svg
					width="20"
					height="20"
					viewBox="0 0 20 20"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
				>
					<path
						d="M0 9.84C0 6.25297 0 4.45946 0.74216 3.10948C1.29067 2.11174 2.11174 1.29067 3.10948 0.74216C4.45946 0 6.25297 0 9.84 0H10.16C13.747 0 15.5405 0 16.8905 0.74216C17.8883 1.29067 18.7093 2.11174 19.2578 3.10948C20 4.45946 20 6.25297 20 9.84V10.16C20 13.747 20 15.5405 19.2578 16.8905C18.7093 17.8883 17.8883 18.7093 16.8905 19.2578C15.5405 20 13.747 20 10.16 20H9.84C6.25297 20 4.45946 20 3.10948 19.2578C2.11174 18.7093 1.29067 17.8883 0.74216 16.8905C0 15.5405 0 13.747 0 10.16V9.84Z"
						fill="url(#paint0_linear_743_30630)"
					/>
					<path
						d="M3.18635 11.7585C2.93788 11.6757 2.93788 11.3243 3.18635 11.2415L5.68497 10.4086C6.49875 10.1373 7.13732 9.49875 7.40858 8.68497L8.24146 6.18635C8.32428 5.93788 8.67572 5.93788 8.75854 6.18635L9.59142 8.68497C9.86268 9.49875 10.5012 10.1373 11.315 10.4086L13.8137 11.2415C14.0621 11.3243 14.0621 11.6757 13.8137 11.7585L11.315 12.5914C10.5012 12.8627 9.86268 13.5012 9.59142 14.315L8.75854 16.8137C8.67572 17.0621 8.32428 17.0621 8.24146 16.8137L7.40858 14.315C7.13732 13.5012 6.49875 12.8627 5.68497 12.5914L3.18635 11.7585Z"
						fill="white"
					/>
					<path
						d="M11.1016 6.14102C10.9661 6.09585 10.9661 5.90415 11.1016 5.85898L12.4645 5.40468C12.9084 5.25672 13.2567 4.90841 13.4047 4.46453L13.859 3.10164C13.9042 2.96612 14.0958 2.96612 14.141 3.10164L14.5953 4.46453C14.7433 4.90841 15.0916 5.25672 15.5355 5.40468L16.8984 5.85898C17.0339 5.90415 17.0339 6.09585 16.8984 6.14102L15.5355 6.59532C15.0916 6.74328 14.7433 7.09159 14.5953 7.53547L14.141 8.89836C14.0958 9.03388 13.9042 9.03388 13.859 8.89836L13.4047 7.53547C13.2567 7.09159 12.9084 6.74328 12.4645 6.59532L11.1016 6.14102Z"
						fill="white"
					/>
					<defs>
						<linearGradient
							id="paint0_linear_743_30630"
							x1="3.5"
							y1="4"
							x2="16"
							y2="15.5"
							gradientUnits="userSpaceOnUse"
						>
							<stop stop-color="#8E48FF" />
							<stop offset="1" stop-color="#FA7269" />
						</linearGradient>
					</defs>
				</svg>
			{/snippet}
			{#snippet title()}
				GitButler features
			{/snippet}

			{#snippet body()}
				Enable automatic creation of branches and automatic generation of commit messages (using
				OpenAI's API).
			{/snippet}
			{#snippet toggle()}
				{#if $user}
					<Toggle
						bind:this={aiGenCheckbox}
						checked={$aiGenEnabled}
						id="aiGenEnabled"
						onclick={() => {
							$aiGenEnabled = !$aiGenEnabled;
						}}
					/>
				{/if}
			{/snippet}

			{#snippet actions()}
				{#if !$user}
					<Login />
				{/if}
			{/snippet}
		</SetupFeature>

		<SetupFeature
			disabled={!$user}
			success={!!$user?.github_access_token}
			topBorder={!!$user && !$user?.github_access_token}
		>
			{#snippet icon()}
				<svg
					width="20"
					height="20"
					viewBox="0 0 20 20"
					xmlns="http://www.w3.org/2000/svg"
					fill="var(--clr-scale-ntrl-0)"
				>
					<path
						fill-rule="evenodd"
						clip-rule="evenodd"
						d="M10.0083 0C4.47396 0 0 4.58331 0 10.2535C0 14.786 2.86662 18.6226 6.84338 19.9805C7.34058 20.0826 7.5227 19.7599 7.5227 19.4885C7.5227 19.2508 7.50631 18.436 7.50631 17.587C4.72225 18.1983 4.14249 16.3647 4.14249 16.3647C3.69508 15.1764 3.03215 14.871 3.03215 14.871C2.12092 14.2429 3.09852 14.2429 3.09852 14.2429C4.1093 14.3108 4.63969 15.2954 4.63969 15.2954C5.53432 16.857 6.97592 16.4158 7.55588 16.1441C7.63865 15.482 7.90394 15.0237 8.18563 14.7691C5.96514 14.5314 3.62891 13.6487 3.62891 9.71017C3.62891 8.58976 4.02634 7.67309 4.65608 6.96018C4.55672 6.7056 4.20866 5.65289 4.75564 4.24394C4.75564 4.24394 5.60069 3.97228 7.5061 5.29644C8.32188 5.07199 9.16317 4.95782 10.0083 4.95685C10.8533 4.95685 11.7148 5.07581 12.5102 5.29644C14.4159 3.97228 15.2609 4.24394 15.2609 4.24394C15.8079 5.65289 15.4596 6.7056 15.3603 6.96018C16.0066 7.67309 16.3876 8.58976 16.3876 9.71017C16.3876 13.6487 14.0514 14.5143 11.8143 14.7691C12.179 15.0916 12.4936 15.7026 12.4936 16.6703C12.4936 18.0453 12.4773 19.1489 12.4773 19.4883C12.4773 19.7599 12.6596 20.0826 13.1566 19.9808C17.1333 18.6224 20 14.786 20 10.2535C20.0163 4.58331 15.526 0 10.0083 0Z"
					/>
				</svg>
			{/snippet}
			{#snippet title()}
				GitHub features
				{#if $user?.github_access_token}
					enabled
					<svg
						class="success-icon"
						width="13"
						height="13"
						viewBox="0 0 13 13"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path
							fill-rule="evenodd"
							clip-rule="evenodd"
							d="M6.5 0C2.91015 0 0 2.91015 0 6.5C0 10.0899 2.91015 13 6.5 13C10.0899 13 13 10.0899 13 6.5C13 2.91015 10.0899 0 6.5 0ZM6.11541 7.12437C6.02319 7.22683 5.86252 7.22683 5.77031 7.12437L4.23194 5.41507L3.19663 6.34684L4.735 8.05614C5.38052 8.77338 6.50519 8.77338 7.15071 8.05614L9.80336 5.10874L8.76806 4.17697L6.11541 7.12437Z"
							fill="#30BB78"
						/>
					</svg>
				{/if}
			{/snippet}
			{#snippet body()}
				Enable creation of pull requests from within the app.
			{/snippet}
			{#snippet actions()}
				{#if !$user?.github_access_token}
					<GithubIntegration disabled={!$user} minimal />
				{/if}
			{/snippet}
		</SetupFeature>
	</div> -->

	<div class="action-buttons">
		<Button kind="outline" onclick={deleteProjectAndGoBack}>Cancel</Button>
		<Button
			style="pop"
			{loading}
			onclick={onSetTargetClick}
			icon="chevron-right-small"
			testId={TestId.ProjectSetupPageTargetContinueButton}
			id="set-base-branch"
		>
			{#if backend.platformName === 'windows'}
				Let's go
			{:else}
				Continue
			{/if}
		</Button>
	</div>
</div>

<style lang="postcss">
	.project-setup {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.project-setup__info {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.project-setup__fields {
		display: flex;
		flex-direction: column;
		padding-bottom: 10px;
		gap: 16px;
	}

	.project-setup__description-text {
		color: var(--clr-text-2);
	}

	.project-setup__field-wrap {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.action-buttons {
		display: flex;
		justify-content: flex-end;
		width: 100%;
		gap: 8px;
	}
	.project-setup__what-the-faq {
		display: flex;
		align-items: center;
		margin-top: 8px;
		gap: 8px;
		cursor: pointer;
		user-select: none;

		&:hover {
			& .project-setup__fold-icon {
				color: var(--clr-text-1);
			}
		}
	}

	.project-setup__fold-icon {
		display: flex;
		color: var(--clr-text-2);
		transition:
			transform var(--transition-medium),
			color var(--transition-fast);

		&.rotate-icon {
			transform: rotate(90deg);
		}
	}
</style>
