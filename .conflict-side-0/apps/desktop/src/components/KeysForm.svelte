<script lang="ts">
	import CredentialCheck from '$components/CredentialCheck.svelte';
	import ProjectNameLabel from '$components/ProjectNameLabel.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Section from '$components/Section.svelte';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { type AuthKey, type KeyType } from '$lib/project/project';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { debounce } from '$lib/utils/debounce';
	import { inject } from '@gitbutler/core/context';
	import { Link, RadioButton, SectionCard, TestId, Textbox } from '@gitbutler/ui';

	import { onMount } from 'svelte';

	interface Props {
		// Used by credential checker before target branch set
		projectId: string;
		remoteName?: string;
		branchName?: string;
		showProjectName?: boolean;
		disabled?: boolean;
	}

	const {
		projectId,
		remoteName = '',
		branchName = '',
		showProjectName = false,
		disabled = false
	}: Props = $props();

	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const baseBranch = $derived(baseBranchQuery.response);
	const projectsService = inject(PROJECTS_SERVICE);
	const projectQuery = $derived(projectsService.getProject(projectId));
	const project = $derived(projectQuery.response);

	let credentialCheck = $state<CredentialCheck>();

	let selectedType: KeyType = $derived(
		typeof project?.preferred_key === 'string' ? project?.preferred_key : 'local'
	);

	let privateKeyPath = $state('');

	async function updateKey(detail: { preferred_key: AuthKey }) {
		try {
			if (project) {
				projectsService.updateProject({ ...project, preferred_key: detail.preferred_key });
			}
		} catch (err: any) {
			showError('Failed to update key', err);
		}
	}

	let form = $state<HTMLFormElement>();

	const debouncedUpdateLocalKey = debounce((privateKey: string) => {
		if (privateKey.trim() === '') {
			updateKey({
				preferred_key: {
					local: {
						private_key_path: privateKey.trim()
					}
				}
			});
		}
	}, 500);

	function onFormChange(selectedType: string) {
		credentialCheck?.reset();
		if (selectedType !== 'local') {
			updateKey({ preferred_key: selectedType as AuthKey });
		} else if (privateKeyPath.trim() !== '') {
			updateKey({
				preferred_key: {
					local: {
						private_key_path: privateKeyPath.trim()
					}
				}
			});
		}
	}

	onMount(async () => {
		if (form) {
			form.credentialType.value = selectedType;
		}
	});
</script>

<div data-testid={TestId.ProjectSetupGitAuthPage}>
	<ReduxResult {projectId} result={projectQuery.result}>
		{#snippet children(project)}
			<Section>
				{#snippet top()}
					{#if showProjectName}<ProjectNameLabel projectName={project.title} />{/if}
				{/snippet}
				{#snippet title()}
					Git authentication
				{/snippet}
				{#snippet description()}
					Configure the authentication flow for GitButler when authenticating with your Git remote
					provider.
				{/snippet}
				<form
					class="git-radio"
					class:disabled
					bind:this={form}
					onchange={(e) => {
						const data = new FormData(e.currentTarget);
						selectedType = data.get('credentialType') as KeyType;
						onFormChange(selectedType);
					}}
				>
					<SectionCard roundedBottom={false} orientation="row" labelFor="git-executable">
						{#snippet title()}
							Use a Git executable <span style="color: var(--clr-text-2)">(default)</span>
						{/snippet}

						{#snippet caption()}
							{#if selectedType === 'systemExecutable'}
								Git executable must be present on your PATH
							{/if}
						{/snippet}

						{#snippet actions()}
							<RadioButton name="credentialType" value="systemExecutable" id="git-executable" />
						{/snippet}
					</SectionCard>

					<SectionCard
						roundedTop={false}
						roundedBottom={false}
						bottomBorder={selectedType !== 'local'}
						orientation="row"
						labelFor="credential-local"
					>
						{#snippet title()}
							Use existing SSH key
						{/snippet}

						{#snippet actions()}
							<RadioButton name="credentialType" id="credential-local" value="local" />
						{/snippet}

						{#snippet caption()}
							{#if selectedType === 'local'}
								Add the path to an existing SSH key that GitButler can use.
							{/if}
						{/snippet}
					</SectionCard>

					{#if selectedType === 'local'}
						<SectionCard topDivider roundedTop={false} roundedBottom={false} orientation="row">
							<div class="inputs-group">
								<Textbox
									label="Path to private key"
									placeholder="for example: ~/.ssh/id_rsa"
									bind:value={privateKeyPath}
									oninput={(value) => {
										debouncedUpdateLocalKey(value);
									}}
								/>
							</div>
						</SectionCard>
					{/if}

					<SectionCard
						roundedTop={false}
						roundedBottom={false}
						orientation="row"
						labelFor="credential-helper"
					>
						{#snippet title()}
							Use a Git credentials helper
						{/snippet}

						{#snippet caption()}
							{#if selectedType === 'gitCredentialsHelper'}
								GitButler will use the system's git credentials helper.
								<Link
									target="_blank"
									rel="noreferrer"
									href="https://git-scm.com/doc/credential-helpers"
								>
									Learn more
								</Link>
							{/if}
						{/snippet}

						{#snippet actions()}
							<RadioButton
								name="credentialType"
								value="gitCredentialsHelper"
								id="credential-helper"
							/>
						{/snippet}
					</SectionCard>

					<SectionCard roundedTop={false} orientation="row">
						<CredentialCheck
							bind:this={credentialCheck}
							disabled={selectedType === 'local' && privateKeyPath.trim() === ''}
							projectId={project.id}
							remoteName={remoteName || baseBranch?.remoteName}
							branchName={branchName || baseBranch?.shortName}
						/>
					</SectionCard>
				</form>
			</Section>
		{/snippet}
	</ReduxResult>
</div>

<style lang="postcss">
	.inputs-group {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 16px;
	}

	.git-radio {
		display: flex;
		flex-direction: column;

		&.disabled {
			opacity: 0.5;
			pointer-events: none;
		}
	}
</style>
