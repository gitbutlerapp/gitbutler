<script lang="ts">
	import { goto } from '$app/navigation';
	import KeyboardShortcutsModal from '$components/KeyboardShortcutsModal.svelte';
	import ShareIssueModal from '$components/ShareIssueModal.svelte';
	import { ircEnabled, codegenEnabled } from '$lib/config/uiFeatureFlags';
	import {
		branchesPath,
		ircPath,
		isBranchesPath,
		isIrcPath,
		isNewProjectSettingsPath,
		isWorkspacePath,
		historyPath,
		isHistoryPath,
		newProjectSettingsPath,
		newSettingsPath,
		workspacePath,
		isCodegenPath,
		codegenPath
	} from '$lib/routes/routes.svelte';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { USER } from '$lib/user/user';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import {
		Button,
		ContextMenu,
		ContextMenuItem,
		ContextMenuSection,
		Icon,
		TestId
	} from '@gitbutler/ui';

	import { slide } from 'svelte/transition';

	const { projectId, disabled = false }: { projectId: string; disabled?: boolean } = $props();

	const user = inject(USER);

	let contextTriggerButton = $state<HTMLButtonElement | undefined>();
	let contextMenuEl = $state<ContextMenu>();
	let shareIssueModal = $state<ShareIssueModal>();
	let keyboardShortcutsModal = $state<KeyboardShortcutsModal>();

	const userService = inject(USER_SERVICE);
	const userSettings = inject(SETTINGS);
</script>

<div class="sidebar">
	<div class="top">
		<div>
			{#if isWorkspacePath()}
				<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
			{/if}
			<Button
				testId={TestId.NavigationWorkspaceButton}
				kind="outline"
				onclick={() => goto(workspacePath(projectId))}
				width={34}
				class={['btn-square', isWorkspacePath() && 'btn-active']}
				tooltip="Workspace"
				{disabled}
			>
				{#snippet custom()}
					<svg viewBox="0 0 16 13" fill="none" xmlns="http://www.w3.org/2000/svg">
						<path
							d="M2 12L3.5 7.5M14 12L12.5 7.5M12.5 7.5L11 3H5L3.5 7.5M12.5 7.5H3.5"
							stroke-width="1.5"
							stroke="var(--clr-workspace-legs)"
						/>
						<path
							d="M1.24142 3H14.7586C14.8477 3 14.8923 2.89229 14.8293 2.82929L13.0293 1.02929C13.0105 1.01054 12.9851 1 12.9586 1H3.04142C3.0149 1 2.98946 1.01054 2.97071 1.02929L1.17071 2.82929C1.10771 2.89229 1.15233 3 1.24142 3Z"
							stroke-width="1.5"
							stroke="var(--clr-workspace-top)"
							fill="var(--clr-workspace-top)"
						/>
					</svg>
				{/snippet}
			</Button>
		</div>
		<div>
			{#if isBranchesPath()}
				<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
			{/if}
			<Button
				testId={TestId.NavigationBranchesButton}
				kind="outline"
				onclick={() => goto(branchesPath(projectId))}
				width={34}
				class={['btn-square', isBranchesPath() && 'btn-active']}
				tooltip="Branches"
				{disabled}
			>
				{#snippet custom()}
					<svg viewBox="0 0 16 14" fill="none" xmlns="http://www.w3.org/2000/svg">
						<path d="M5 3L11 3" stroke-width="1.5" stroke="var(--clr-branches)" />
						<path
							d="M3 5L3 7.17157C3 7.70201 3.21071 8.21071 3.58579 8.58579L5.41421 10.4142C5.78929 10.7893 6.29799 11 6.82843 11L11.5 11"
							stroke-width="1.5"
							stroke="var(--clr-branches)"
						/>
						<rect
							x="15"
							y="1"
							width="4"
							height="4"
							transform="rotate(90 15 1)"
							stroke-width="1.5"
							fill="var(--clr-branches)"
							stroke="var(--clr-branches)"
						/>
						<rect
							x="15"
							y="9"
							width="4"
							height="4"
							transform="rotate(90 15 9)"
							stroke-width="1.5"
							fill="var(--clr-branches)"
							stroke="var(--clr-branches)"
						/>
						<rect
							x="5"
							y="1"
							width="4"
							height="4"
							transform="rotate(90 5 1)"
							stroke-width="1.5"
							fill="var(--clr-branches)"
							stroke="var(--clr-branches)"
						/>
					</svg>
				{/snippet}
			</Button>
		</div>
		{#if $codegenEnabled}
			<div>
				{#if isCodegenPath()}
					<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
				{/if}
				<Button
					testId={TestId.NavigationBranchesButton}
					kind="outline"
					onclick={() => goto(codegenPath(projectId))}
					width={34}
					class={['btn-square', isCodegenPath() && 'btn-active']}
					tooltip="Codegen"
					{disabled}
				>
					{#snippet custom()}
						<!-- tehehe butthole -->
						<svg
							xmlns="http://www.w3.org/2000/svg"
							width="20"
							height="20"
							viewBox="0 0 20 20"
							fill="none"
						>
							<path
								opacity="0.7"
								d="M3.92333 13.2994L7.85857 11.0927L7.92476 10.9009L7.85857 10.7943H7.66667L7.00905 10.7538L4.76048 10.6929L2.81048 10.612L0.920952 10.5106L0.445714 10.4093L0 9.82249L0.0457143 9.52982L0.445238 9.26141L1.0181 9.31138L4.18238 9.52886L5.56 9.60977L7.60048 9.82154H7.92476L7.97048 9.69067L7.85952 9.60977L7.77286 9.52886L5.80762 8.19826L3.68048 6.79199L2.56619 5.98201L1.96381 5.57179L1.66 5.18726L1.52905 4.34778L2.07619 3.74578L2.81095 3.79575L2.99857 3.84571L3.74286 4.41822L5.33238 5.64793L7.40809 7.17556L7.7119 7.42778L7.83333 7.34117L7.8481 7.28025L7.7119 7.0523L6.58286 5.01309L5.37809 2.93866L4.84286 2.07824L4.70095 1.56284C4.65095 1.35107 4.61428 1.17261 4.61428 0.955599L5.23667 0.110884L5.58095 0L6.41143 0.110884L6.7619 0.414029L7.27762 1.59378L8.11381 3.45072L9.41048 5.97582L9.79 6.72488L9.99238 7.41874L10.0681 7.63051H10.199V7.50916L10.3057 6.08671L10.5029 4.34017L10.6948 2.09299L10.761 1.46005L11.0743 0.70147L11.6971 0.291248L12.1833 0.523485L12.5829 1.09599L12.5276 1.46576L12.29 3.01004L11.8243 5.42854L11.5205 7.04802H11.6976L11.9 6.84576L12.72 5.75786L14.0976 4.03702L14.7052 3.35411L15.4143 2.59982L15.8695 2.24099H16.73L17.3633 3.18184L17.0795 4.15362L16.1938 5.27673L15.459 6.22805L14.4057 7.64527L13.7481 8.77885L13.809 8.86927L13.9657 8.85452L16.3452 8.34817L17.631 8.11593L19.1652 7.85276L19.8595 8.17684L19.9352 8.50616L19.6624 9.17956L18.0214 9.58454L16.0971 9.96907L13.2314 10.6467L13.1962 10.6724L13.2367 10.7224L14.5276 10.8438L15.08 10.8733H16.4319L18.9495 11.0608L19.6071 11.4953L20 12.0268L19.9338 12.4318L18.921 12.9472L14.3643 11.8646L13.2705 11.5919H13.119V11.6823L14.0305 12.5732L15.701 14.0808L17.7929 16.0244L17.8995 16.505L17.631 16.8843L17.3471 16.8439L15.509 15.4619L14.8 14.8399L13.1943 13.4888H13.0876V13.6306L13.4576 14.1717L15.4119 17.1075L15.5133 18.0079L15.3714 18.3006L14.8648 18.4776L14.3086 18.3762L13.1648 16.7715L11.9843 14.9641L11.0324 13.3446L10.9162 13.4107L10.3543 19.4589L10.091 19.7678L9.48333 20L8.97667 19.6155L8.7081 18.9935L8.97667 17.7638L9.30095 16.159L9.56429 14.8836L9.8019 13.2989L9.94381 12.7726L9.93429 12.7374L9.8181 12.7521L8.62286 14.392L6.80524 16.8472L5.36714 18.3858L5.02286 18.5219L4.42571 18.213L4.48095 17.661L4.81476 17.1698L6.80524 14.639L8.00571 13.0705L8.78095 12.1649L8.77571 12.034H8.73L3.44286 15.4647L2.50143 15.5861L2.09619 15.2068L2.14619 14.5848L2.3381 14.3825L3.92762 13.2894L3.92238 13.2946L3.92333 13.2994Z"
								fill="var(--clr-claude-bg)"
								stroke="none"
							/>
						</svg>
					{/snippet}
				</Button>
			</div>
		{/if}
		<div>
			{#if isHistoryPath()}
				<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
			{/if}
			<Button
				kind="outline"
				onclick={() => goto(historyPath(projectId))}
				width={34}
				class={['btn-square', isHistoryPath() && 'btn-active']}
				tooltip="Operations history"
				tooltipHotkey="⇧⌘H"
				{disabled}
			>
				{#snippet custom()}
					<svg
						width="18"
						height="18"
						viewBox="0 0 18 18"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						{#if !isHistoryPath()}
							<path
								d="M7 1H5C2.79086 1 1 2.79086 1 5V13C1 15.2091 2.79086 17 5 17H13C15.2091 17 17 15.2091 17 13V11"
								stroke-width="1.5"
							/>
							<path
								d="M17 11V5C17 2.79086 15.2091 1 13 1H7"
								stroke-width="1.5"
								stroke-dasharray="1.5 1.5"
							/>
						{:else}
							<rect
								x="1"
								y="1"
								width="16"
								height="16"
								rx="4"
								fill="var(--clr-history-bg)"
								stroke="var(--clr-history-bg)"
								stroke-width="1.5"
							/>
						{/if}
						<path d="M8 4V10H14" stroke="var(--clr-history-arrows)" stroke-width="1.5" />
					</svg>
				{/snippet}
			</Button>
		</div>
		{#if $ircEnabled}
			<div>
				{#if isIrcPath()}
					<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
				{/if}
				<Button
					kind="outline"
					onclick={() => goto(ircPath(projectId))}
					icon="chat"
					width={34}
					class={['btn-square', isIrcPath() && 'btn-active']}
					tooltip="History"
					{disabled}
				/>
			</div>
		{/if}
	</div>
	<div class="bottom">
		<div class="bottom__primary-actions">
			<div>
				{#if isNewProjectSettingsPath()}
					<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
				{/if}
				<Button
					kind="outline"
					onclick={() => {
						if (isNewProjectSettingsPath()) {
							goto(workspacePath(projectId));
						} else {
							goto(newProjectSettingsPath(projectId));
						}
					}}
					width={34}
					class={['btn-square', isNewProjectSettingsPath() && 'btn-active']}
					tooltipPosition="top"
					tooltipAlign="start"
					tooltip="Project settings"
				>
					{#snippet custom()}
						<svg
							width="14"
							height="16"
							viewBox="0 0 14 16"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path
								fill-rule="evenodd"
								clip-rule="evenodd"
								d="M8.06218 1.26795C7.44338 0.910684 6.68098 0.910684 6.06218 1.26795L2 3.61325C1.3812 3.97051 1 4.63077 1 5.3453V10.0359C1 10.7504 1.3812 11.4107 2 11.7679L6.06218 14.1132C6.68098 14.4705 7.44338 14.4705 8.06218 14.1132L12.1244 11.7679C12.7432 11.4107 13.1244 10.7504 13.1244 10.0359V5.3453C13.1244 4.63077 12.7432 3.97051 12.1244 3.61325L8.06218 1.26795ZM8.06218 5.26795C7.44338 4.91068 6.68098 4.91068 6.06218 5.26795L5.4641 5.61325C4.8453 5.97051 4.4641 6.63077 4.4641 7.3453V8.0359C4.4641 8.75043 4.8453 9.41068 5.4641 9.76795L6.06218 10.1132C6.68098 10.4705 7.44338 10.4705 8.06218 10.1132L8.66025 9.76795C9.27906 9.41068 9.66025 8.75043 9.66025 8.0359V7.3453C9.66025 6.63077 9.27906 5.97051 8.66025 5.61325L8.06218 5.26795Z"
								stroke="var(--clr-settings-bg)"
								fill="var(--clr-settings-bg)"
								stroke-width="1.5"
							/>
						</svg>
					{/snippet}
				</Button>
			</div>

			<Button
				kind="outline"
				width={34}
				class="btn-height-auto"
				onclick={() => {
					contextMenuEl?.toggle();
				}}
				bind:el={contextTriggerButton}
			>
				{#snippet custom()}
					<div class="user-button">
						<div class="user-icon">
							{#if $user?.picture}
								<img
									class="user-icon__image"
									src={$user.picture}
									alt=""
									referrerpolicy="no-referrer"
								/>
							{:else}
								<Icon name="profile" />
							{/if}
						</div>
						<div class="user-button__select-icon">
							<Icon name="select-chevron" />
						</div>
					</div>
				{/snippet}
			</Button>
		</div>
		<div class="bottom__ghost-actions">
			<Button
				icon="keyboard"
				kind="ghost"
				style="neutral"
				tooltip="Keyboard Shortcuts (Coming soon...)"
				tooltipPosition="top"
				tooltipAlign="start"
				width={34}
				class="faded-btn"
				onclick={() => {
					keyboardShortcutsModal?.show();
				}}
				disabled={true}
			/>
			<Button
				icon="mail"
				kind="ghost"
				tooltip="Share feedback"
				tooltipPosition="top"
				tooltipAlign="start"
				width={34}
				class="faded-btn"
				onclick={() => {
					shareIssueModal?.show();
				}}
			/>
		</div>
	</div>
</div>

<ContextMenu
	bind:this={contextMenuEl}
	leftClickTrigger={contextTriggerButton}
	side="right"
	align="start"
>
	<ContextMenuSection>
		<ContextMenuItem
			label="Global settings"
			onclick={() => {
				goto(newSettingsPath());
				contextMenuEl?.close();
			}}
			keyboardShortcut="⌘,"
		/>
	</ContextMenuSection>
	<ContextMenuSection title="Theme (⌘T)">
		<ContextMenuItem
			label="Dark"
			onclick={async () => {
				userSettings.update((s) => ({
					...s,
					theme: 'dark'
				}));
				contextMenuEl?.close();
			}}
		/>
		<ContextMenuItem
			label="Light"
			onclick={async () => {
				userSettings.update((s) => ({
					...s,
					theme: 'light'
				}));
				contextMenuEl?.close();
			}}
		/>
		<ContextMenuItem
			label="System"
			onclick={async () => {
				userSettings.update((s) => ({
					...s,
					theme: 'system'
				}));
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
	{#if $user}
		<ContextMenuSection>
			<ContextMenuItem
				label="Log out"
				onclick={async () => {
					await userService.logout();
				}}
			/>
		</ContextMenuSection>
	{/if}
</ContextMenu>

<KeyboardShortcutsModal bind:this={keyboardShortcutsModal} />
<ShareIssueModal bind:this={shareIssueModal} />

<style lang="postcss">
	.sidebar {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		justify-content: space-between;
		height: 100%;
		padding: 0 16px 16px 16px;
	}

	.top,
	.bottom {
		display: flex;
		flex-direction: column;
	}
	.top {
		gap: 4px;
	}
	.bottom {
		gap: 16px;
	}
	.bottom__primary-actions {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.bottom__ghost-actions {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	/* USER BUTTON */
	.user-button {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		width: 34px;
		padding: 4px;
		gap: 4px;
	}

	.user-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 26px;
		height: 26px;
		overflow: hidden;
		border-radius: var(--radius-m);
		background-color: var(--clr-core-pop-50);
		color: var(--clr-core-ntrl-100);
	}

	.user-icon__image {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.user-button__select-icon {
		color: var(--label-clr);
		opacity: var(--icon-opacity);
		transition:
			opacity var(--transition-fast),
			color var(--transition-fast);
	}

	.active-page-indicator {
		position: absolute;
		left: 0;
		width: 12px;
		height: 18px;
		transform: translateX(-50%) translateY(50%);
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-pop-element);
		content: '';
	}

	/* OVERRIDE BUTTON STYLES */
	:global(.sidebar .btn-square) {
		--label-clr: var(--clr-btn-ntrl-outline-text);
		--icon-opacity: var(--opacity-btn-icon-outline);
		--clr-claude-bg: var(--clr-btn-ntrl-outline-text);
		& svg {
			width: 16px;
			height: 16px;
			stroke: currentColor;
			opacity: var(--icon-opacity);
			transition: opacity var(--transition-fast);
		}

		&:hover {
			--icon-opacity: var(--opacity-btn-icon-outline-hover);
		}
	}
	:global(.sidebar .btn-height-auto) {
		height: auto;
		padding: 0;
		border-radius: var(--radius-ml);
	}
	:global(.sidebar .btn-square) {
		aspect-ratio: 1 / 1;
		height: unset;
		border-radius: var(--radius-ml);
	}
	:global(.sidebar .btn-square.btn-active) {
		--icon-opacity: 1;
		--btn-bg: var(--clr-bg-1);
		--opacity-btn-bg: 1;

		/* workspace icon */
		--clr-workspace-legs: #d96842;
		--clr-workspace-top: #ff9774;

		/* branches icon */
		--clr-branches: #a486c8;

		/* target icon */
		--clr-target-bg: #ff9774;
		--clr-target-lines: #fff;

		/* history icon */
		--clr-history-bg: #fbdb79;
		--clr-history-arrows: #0a0a0a;

		/* settings icon */
		--clr-settings-bg: var(--label-clr);

		/* claude icon */
		--clr-claude-bg: #da6742;
	}

	:global(.sidebar .faded-btn) {
		--icon-opacity: 0.4;

		&:hover {
			--icon-opacity: 0.6;
		}
	}
</style>
