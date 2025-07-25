<script lang="ts">
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';

	import { Button, LinkButton, Modal, Textbox } from '@gitbutler/ui';
	import { error } from '@gitbutler/ui/toasts';
	import { slugify } from '@gitbutler/ui/utils/string';

	interface Props {
		projectId: string;
		stackId: string;
	}

	const BRANCH_STACKING_DOCS =
		'https://docs.gitbutler.com/features/virtual-branches/stacked-branches';
	function clickOnDocsLink() {
		openExternalUrl(BRANCH_STACKING_DOCS);
	}

	const { projectId, stackId }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const [createNewBranch, branchCreation] = stackService.newBranch;

	let createRefModal = $state<ReturnType<typeof Modal>>();
	let createRefName: string | undefined = $state();
	let parentBranch: string | undefined = $state();

	const slugifiedRefName = $derived(createRefName && slugify(createRefName));
	const generatedNameDiverges = $derived(!!createRefName && slugifiedRefName !== createRefName);

	async function addSeries() {
		if (!slugifiedRefName) {
			error('No branch name provided');
			createRefModal?.close();
			return;
		}

		await createNewBranch({
			projectId,
			stackId,
			request: { targetPatch: undefined, name: slugifiedRefName }
		});

		createRefModal?.close();
	}

	function onModalClose() {
		createRefName = undefined;
	}

	export function show(branchName?: string) {
		parentBranch = branchName;
		createRefModal?.show();
	}
</script>

<Modal
	bind:this={createRefModal}
	title="Add branch to the stack"
	width="small"
	onSubmit={addSeries}
	onClose={onModalClose}
>
	<Textbox
		label="Branch name"
		id="newRemoteName"
		bind:value={createRefName}
		autofocus
		helperText={generatedNameDiverges ? `Will be created as '${slugifiedRefName}'` : undefined}
	/>

	<p class="text-12 text-body helper-text">
		Creates a new branch that depends on <strong>{parentBranch}</strong>. The branches will have to
		be reviewed and merged in order. Learn more about stacked branches in the
		<LinkButton onclick={clickOnDocsLink}>docs</LinkButton>.
	</p>
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button
			style="pop"
			type="submit"
			disabled={!createRefName}
			loading={branchCreation.current.isLoading}>Add new branch</Button
		>
	{/snippet}
</Modal>

<style>
	.helper-text {
		margin-top: 10px;
		color: var(--clr-scale-ntrl-50);
	}
</style>
