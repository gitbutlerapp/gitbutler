<script lang="ts">
	import { page } from "$app/state";
	import { BACKUP_SERVICE } from "$lib/backups/backupService.svelte";
	import { BRANCH_SERVICE } from "$lib/branches/branchService.svelte";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		Badge,
		Button,
		Checkbox,
		EmptyStatePlaceholder,
		Modal,
		Textbox,
		VirtualList,
	} from "@gitbutler/ui";

	const projectId = $derived(page.params.projectId!);
	const backupService = inject(BACKUP_SERVICE);
	const branchService = inject(BRANCH_SERVICE);
	const uiState = inject(UI_STATE);

	const backupsQuery = $derived(backupService.backups(projectId));
	const settingsQuery = $derived(backupService.settings(projectId));
	const branchesQuery = $derived(branchService.list(projectId));
	const backups = $derived(backupsQuery.response ?? []);
	const settings = $derived(settingsQuery.response);

	let selectedBackupId = $state<string | undefined>();
	let selectedRefName = $state<string | undefined>();
	let selectedFiles = $state<Set<string>>(new Set());
	let createModal = $state<Modal>();
	let settingsModal = $state<Modal>();
	let message = $state("");
	let creating = $state(false);
	let deleting = $state(false);
	let verifying = $state(false);
	let restoringBranch = $state(false);
	let restoringFiles = $state(false);
	let verifyMessage = $state<string | undefined>();
	let restoreBranchName = $state("");
	let overwriteBranch = $state(false);
	let backupDirectory = $state("");
	let previewPath = $state<string | undefined>();
	let previewDiff = $state<string | undefined>();
	let previewLoading = $state(false);
	let previewError = $state<string | undefined>();

	const selectedBackup = $derived(
		backups.find((backup) => backup.id === selectedBackupId) ?? backups[0],
	);
	const refsQuery = $derived(backupService.refs(projectId, selectedBackup?.id));
	const refs = $derived(refsQuery.response ?? []);
	const selectedRef = $derived(refs.find((ref) => ref.name === selectedRefName) ?? refs[0]);
	const filesQuery = $derived(backupService.files(projectId, selectedBackup?.id, selectedRef?.name));
	const files = $derived(filesQuery.response ?? []);
	const branchNames = $derived((branchesQuery.response ?? []).map((branch) => branch.name));
	let selectedBranches = $state<Set<string>>(new Set());

	$effect(() => {
		if (!selectedBackupId && backups[0]) selectedBackupId = backups[0].id;
	});

	$effect(() => {
		if (selectedRef && selectedRefName !== selectedRef.name) {
			selectedRefName = selectedRef.name;
			restoreBranchName = shortBranchName(selectedRef.name);
			selectedFiles = new Set();
			resetPreview();
		}
	});

	$effect(() => {
		if (settings) backupDirectory = settings.backupDirectory;
	});

	function openCreateModal() {
		message = "";
		selectedBranches = new Set(branchNames.slice(0, 1));
		createModal?.show();
	}

	async function createBackup(close: () => void) {
		if (creating) return;
		creating = true;
		try {
			await backupService.createBackup({
				projectId,
				branchNames: Array.from(selectedBranches),
				message,
				reason: "manual",
			});
			close();
		} finally {
			creating = false;
		}
	}

	async function deleteSelectedBackup() {
		if (!selectedBackup || deleting) return;
		deleting = true;
		try {
			await backupService.deleteBackup(projectId, selectedBackup.id);
			selectedBackupId = undefined;
		} finally {
			deleting = false;
		}
	}

	async function verifySelectedBackup() {
		if (!selectedBackup || verifying) return;
		verifying = true;
		try {
			const result = await backupService.verifyBackup(projectId, selectedBackup.id);
			verifyMessage = result.valid ? "Backup verified" : result.message || "Backup verification failed";
		} finally {
			verifying = false;
		}
	}

	async function previewFile(path: string) {
		if (!selectedBackup || !selectedRef || previewLoading) return;
		previewPath = path;
		previewDiff = undefined;
		previewError = undefined;
		previewLoading = true;
		try {
			const preview = await backupService.previewFile({
				projectId,
				backupId: selectedBackup.id,
				refName: selectedRef.name,
				path,
			});
			previewDiff = preview.diff || "No changes from the current worktree file.";
		} catch (error) {
			previewError = error instanceof Error ? error.message : String(error);
		} finally {
			previewLoading = false;
		}
	}

	async function restoreSelectedBranch() {
		if (!selectedBackup || !selectedRef || !restoreBranchName || restoringBranch) return;
		restoringBranch = true;
		try {
			await backupService.restoreBranch({
				projectId,
				backupId: selectedBackup.id,
				refName: selectedRef.name,
				targetBranchName: restoreBranchName,
				overwrite: overwriteBranch,
			});
		} finally {
			restoringBranch = false;
		}
	}

	async function restoreSelectedFiles() {
		if (!selectedBackup || !selectedRef || selectedFiles.size === 0 || restoringFiles) return;
		restoringFiles = true;
		try {
			await backupService.restoreFiles({
				projectId,
				backupId: selectedBackup.id,
				refName: selectedRef.name,
				paths: Array.from(selectedFiles),
			});
		} finally {
			restoringFiles = false;
		}
	}

	async function chooseDirectory() {
		const picked = await backupService.chooseBackupDirectory(backupDirectory);
		if (picked) backupDirectory = picked;
	}

	async function saveSettings(close: () => void) {
		if (!settings) return;
		await backupService.updateSettings(projectId, {
			backupDirectory,
			backupBeforeUpstreamDefault: settings.backupBeforeUpstreamDefault,
		});
		close();
	}

	function toggleBranch(name: string) {
		const next = new Set(selectedBranches);
		if (next.has(name)) {
			next.delete(name);
		} else {
			next.add(name);
		}
		selectedBranches = next;
	}

	function toggleFile(path: string) {
		const next = new Set(selectedFiles);
		if (next.has(path)) {
			next.delete(path);
		} else {
			next.add(path);
		}
		selectedFiles = next;
	}

	function resetPreview() {
		previewPath = undefined;
		previewDiff = undefined;
		previewError = undefined;
		previewLoading = false;
	}

	function diffLineClass(line: string) {
		if (line.startsWith("+++") || line.startsWith("---")) return "diff-line--file";
		if (line.startsWith("@@")) return "diff-line--hunk";
		if (line.startsWith("+")) return "diff-line--added";
		if (line.startsWith("-")) return "diff-line--removed";
		return "";
	}

	function formatDate(ms: number) {
		return new Date(Number(ms)).toLocaleString();
	}

	function formatSize(bytes: number) {
		if (bytes < 1024) return `${bytes} B`;
		if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
		return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	}

	function shortBranchName(refName: string) {
		return refName.replace(/^refs\/heads\//, "");
	}
</script>

<div class="backups-view">
	<section class="backup-list">
		<div class="backup-list__header">
			<div>
				<h3 class="text-15 text-bold">Backups</h3>
				<p class="text-12 text-body">{settings?.backupDirectory}</p>
			</div>
			<div class="header-actions">
				<Button kind="outline" size="tag" icon="settings" onclick={() => settingsModal?.show()}>
					Folder
				</Button>
				<Button kind="outline" size="tag" icon="plus" onclick={openCreateModal}>
					Create backup
				</Button>
			</div>
		</div>

		{#if backups.length === 0}
			<EmptyStatePlaceholder bottomMargin={48}>
				{#snippet title()}No backups yet{/snippet}
				{#snippet caption()}Create a branch bundle backup to recover branches or files later.{/snippet}
			</EmptyStatePlaceholder>
		{:else}
			<VirtualList
				grow
				items={backups}
				defaultHeight={66}
				renderDistance={180}
				visibility={uiState.global.scrollbarVisibilityState.current}
				getId={(backup) => backup.id}
			>
				{#snippet template(backup)}
						<button
							type="button"
							class="backup-card"
							class:selected={selectedBackup?.id === backup.id}
							onclick={() => {
								selectedBackupId = backup.id;
								selectedRefName = undefined;
								verifyMessage = undefined;
								selectedFiles = new Set();
								resetPreview();
							}}
						>
							<span class="backup-card__title text-13 text-semibold">
								{backup.message || backup.reason || backup.id}
							</span>
							<span class="backup-card__meta text-12">
								{formatDate(backup.createdAt)} • {backup.branches.length} branches • {formatSize(backup.size)}
							</span>
						</button>
				{/snippet}
			</VirtualList>
		{/if}
	</section>

	<section class="backup-detail">
		{#if selectedBackup}
			<div class="detail-header">
				<div>
					<h3 class="text-15 text-bold">{selectedBackup.message || selectedBackup.id}</h3>
					<p class="text-12 text-body">{selectedBackup.bundlePath}</p>
				</div>
				<div class="header-actions">
					<Button kind="outline" size="tag" loading={verifying} onclick={verifySelectedBackup}>
						Verify
					</Button>
					<Button kind="outline" size="tag" loading={deleting} onclick={deleteSelectedBackup}>
						Delete
					</Button>
				</div>
			</div>

			{#if verifyMessage}
				<div class="notice text-12">{verifyMessage}</div>
			{/if}

			<div class="detail-grid">
				<div class="refs-panel">
					<h4 class="text-13 text-semibold">Branches</h4>
					{#each refs as ref (ref.name)}
						<button
							type="button"
							class="ref-row"
							class:selected={selectedRef?.name === ref.name}
							onclick={() => {
								selectedRefName = ref.name;
								restoreBranchName = shortBranchName(ref.name);
								selectedFiles = new Set();
								resetPreview();
							}}
						>
							<span>{shortBranchName(ref.name)}</span>
							<span class="text-11">{ref.sha.slice(0, 7)}</span>
						</button>
					{/each}
				</div>

				<div class="files-panel">
					<div class="files-header">
						<h4 class="text-13 text-semibold">Files</h4>
						<Badge>{files.length}</Badge>
					</div>
					<VirtualList
						grow
						items={files}
						defaultHeight={34}
						renderDistance={180}
						visibility={uiState.global.scrollbarVisibilityState.current}
						getId={(file) => file}
					>
						{#snippet template(file)}
							<div class="file-row" class:selected={previewPath === file}>
								<Checkbox small checked={selectedFiles.has(file)} onchange={() => toggleFile(file)} />
								<button type="button" class="file-preview-trigger text-12" onclick={() => previewFile(file)}>
									{file}
								</button>
							</div>
						{/snippet}
					</VirtualList>
				</div>

				<div class="preview-panel">
					<div class="preview-header">
						<h4 class="text-13 text-semibold">Preview</h4>
						{#if previewLoading}
							<Badge>Loading</Badge>
						{/if}
					</div>
					{#if previewPath}
						<div class="preview-path text-12">{previewPath}</div>
						{#if previewError}
							<div class="preview-error text-12">{previewError}</div>
						{:else if previewDiff}
							<pre class="diff-preview text-12">{#each previewDiff.split("\n") as line}<span class={diffLineClass(line)}>{line}</span>
{/each}</pre>
						{:else}
							<div class="preview-empty text-12">
								Select a file to compare the backup copy with the current worktree.
							</div>
						{/if}
					{:else}
						<div class="preview-empty text-12">
							Select a file to compare the backup copy with the current worktree.
						</div>
					{/if}
				</div>
			</div>

			<div class="restore-panel">
				<Textbox placeholder="Branch name" bind:value={restoreBranchName} />
				<label class="inline-check text-12">
					<Checkbox small bind:checked={overwriteBranch} />
					Overwrite existing branch
				</label>
				<Button style="pop" loading={restoringBranch} onclick={restoreSelectedBranch}>
					Restore branch
				</Button>
				<Button
					kind="outline"
					loading={restoringFiles}
					disabled={selectedFiles.size === 0}
					onclick={restoreSelectedFiles}
				>
					Restore selected files
				</Button>
			</div>
		{:else}
			<EmptyStatePlaceholder bottomMargin={48}>
				{#snippet title()}Select a backup{/snippet}
				{#snippet caption()}Backups appear here after you create your first Git bundle.{/snippet}
			</EmptyStatePlaceholder>
		{/if}
	</section>
</div>

<Modal title="Create backup" bind:this={createModal} width="small" onSubmit={createBackup}>
	<div class="modal-stack">
		<Textbox placeholder="Backup description (optional)" bind:value={message} autofocus />
		<div class="branch-picker">
			{#each branchNames as branch}
				<label class="picker-row">
					<Checkbox small checked={selectedBranches.has(branch)} onchange={() => toggleBranch(branch)} />
					<span class="text-12">{branch}</span>
				</label>
			{/each}
		</div>
		<p class="text-12 text-body">Destination: {settings?.backupDirectory}</p>
	</div>
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" type="submit" loading={creating} disabled={selectedBranches.size === 0}>
			Create backup
		</Button>
	{/snippet}
</Modal>

<Modal title="Backup folder" bind:this={settingsModal} width="small" onSubmit={saveSettings}>
	<div class="modal-stack">
		<Textbox placeholder="Backup folder" bind:value={backupDirectory} />
		<Button kind="outline" onclick={chooseDirectory}>Choose folder</Button>
	</div>
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" type="submit">Save</Button>
	{/snippet}
</Modal>

<style>
	.backups-view {
		display: grid;
		grid-template-columns: minmax(360px, 34%) minmax(0, 1fr);
		width: 100%;
		height: 100%;
		gap: 8px;
		overflow: hidden;
	}

	.backup-list,
	.backup-detail {
		display: flex;
		flex-direction: column;
		min-width: 0;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-ml);
		background: var(--bg-1);
	}

	.backup-list__header,
	.detail-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		padding: 12px 14px;
		gap: 12px;
		border-bottom: 1px solid var(--border-2);
	}

	.header-actions,
	.restore-panel {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.backup-cards {
		display: flex;
		flex-direction: column;
	}

	.backup-card,
	.ref-row {
		display: flex;
		flex-direction: column;
		padding: 12px 14px;
		gap: 5px;
		border-bottom: 1px solid var(--border-2);
		text-align: left;
	}

	.backup-card:hover,
	.ref-row:hover,
	.backup-card.selected,
	.ref-row.selected {
		background: var(--focus-bg-mute);
	}

	.backup-card__meta,
	.detail-header p,
	.backup-list__header p,
	.ref-row span:last-child {
		overflow: hidden;
		color: var(--text-2);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.notice {
		margin: 12px 14px 0;
		padding: 8px 10px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-s);
		background: var(--bg-2);
	}

	.detail-grid {
		display: grid;
		grid-template-columns: 220px minmax(260px, 34%) minmax(0, 1fr);
		min-height: 0;
		flex: 1;
		overflow: hidden;
	}

	.refs-panel,
	.files-panel {
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
		overflow: hidden;
		border-right: 1px solid var(--border-2);
	}

	.refs-panel h4,
	.files-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 14px;
		border-bottom: 1px solid var(--border-2);
	}

	.preview-panel,
	.files-panel {
		border-right: 1px solid var(--border-2);
	}

	.preview-panel {
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
		overflow: hidden;
		border-right: none;
	}

	.branch-picker,
	.modal-stack {
		display: flex;
		flex-direction: column;
	}

	.file-row,
	.picker-row,
	.inline-check {
		display: flex;
		align-items: center;
		min-width: 0;
		padding: 8px 12px;
		gap: 8px;
	}

	.file-row.selected {
		background: var(--focus-bg-mute);
	}

	.file-preview-trigger {
		min-width: 0;
		overflow: hidden;
		text-align: left;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.picker-row span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.preview-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 14px;
		border-bottom: 1px solid var(--border-2);
	}

	.preview-path {
		padding: 8px 14px;
		overflow: hidden;
		border-bottom: 1px solid var(--border-2);
		color: var(--text-2);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.preview-empty,
	.preview-error {
		padding: 14px;
		color: var(--text-2);
	}

	.preview-error {
		color: var(--clr-scale-ntrl-60);
	}

	.diff-preview {
		flex: 1;
		min-height: 0;
		margin: 0;
		overflow: auto;
		padding: 10px 0;
		background: var(--bg-1);
		font-family: var(--fontfamily-mono);
		line-height: 1.45;
		white-space: pre;
	}

	.diff-preview span {
		display: block;
		padding: 0 14px;
	}

	.diff-line--file,
	.diff-line--hunk {
		color: var(--text-2);
	}

	.diff-line--added {
		background: var(--diff-addition-line-bg);
		color: var(--diff-addition-count-text);
	}

	.diff-line--removed {
		background: var(--diff-deletion-line-bg);
		color: var(--diff-deletion-count-text);
	}

	.restore-panel {
		padding: 12px 14px;
		border-top: 1px solid var(--border-2);
	}

	.modal-stack {
		gap: 12px;
	}

	.branch-picker {
		max-height: 240px;
		overflow: auto;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-s);
	}
</style>
