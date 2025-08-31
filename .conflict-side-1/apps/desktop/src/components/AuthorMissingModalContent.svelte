<script lang="ts">
	import { GIT_SERVICE } from '$lib/git/gitService';
	import { inject } from '@gitbutler/shared/context';
	import { TestId, ModalHeader, ModalFooter, Textbox, EmailTextbox, Button } from '@gitbutler/ui';
	import type { AuthorMissingModalState } from '$lib/state/uiState.svelte';

	type Props = {
		data: AuthorMissingModalState;
		close: () => void;
	};

	const { data, close }: Props = $props();

	const gitService = inject(GIT_SERVICE);
	const [setAuthorInfo, settingInfo] = gitService.setAuthorInfo;

	let name = $derived(data.authorName);
	let email = $derived(data.authorEmail);
	let emailTextbox: any;

	async function handleSubmit() {
		if (!name || !email) {
			return;
		}
		if (!emailTextbox.isValid()) {
			emailTextbox.validate();
			return;
		}
		await setAuthorInfo({
			projectId: data.projectId,
			name,
			email
		});
		close();
	}
</script>

<ModalHeader type="warning">Set up your git author information</ModalHeader>
<div class="author-missing__content">
	Your commits need author information to identify who made the changes. This information will be
	saved to your global git configuration and used for all future commits.

	<Textbox
		disabled={settingInfo.current.isLoading}
		placeholder="Your full name"
		label="Name"
		testId={TestId.GlobalModal_AuthorMissing_NameInput}
		bind:value={name}
		autofocus
	/>

	<EmailTextbox
		disabled={settingInfo.current.isLoading}
		placeholder="your.email@example.com"
		label="Email address"
		testId={TestId.GlobalModal_AuthorMissing_EmailInput}
		bind:value={email}
		bind:this={emailTextbox}
	/>
</div>
<ModalFooter>
	<Button kind="outline" onclick={close} disabled={settingInfo.current.isLoading}>Cancel</Button>
	<Button
		testId={TestId.GlobalModal_AuthorMissing_ActionButton}
		style="pop"
		onclick={handleSubmit}
		loading={settingInfo.current.isLoading}
		disabled={!name || !email}
	>
		{settingInfo.current.isLoading ? 'Saving...' : 'Save & Continue'}
	</Button>
</ModalFooter>

<style lang="postcss">
	.author-missing__content {
		display: flex;
		flex-direction: column;
		padding: 0 16px 16px 16px;
		gap: 16px;
	}
</style>
