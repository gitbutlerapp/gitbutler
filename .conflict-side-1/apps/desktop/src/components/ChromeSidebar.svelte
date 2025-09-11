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
		isWorkspacePath,
		historyPath,
		isHistoryPath,
		workspacePath,
		isCodegenPath,
		codegenPath
	} from '$lib/routes/routes.svelte';
	import { useSettingsModal } from '$lib/settings/settingsModal.svelte';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { USER } from '$lib/user/user';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import {
		Button,
		ContextMenu,
		ContextMenuItem,
		ContextMenuSection,
		Icon,
		TestId
	} from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';

	import { slide } from 'svelte/transition';

	const { projectId, disabled = false }: { projectId: string; disabled?: boolean } = $props();

	const user = inject(USER);

	let contextTriggerButton = $state<HTMLButtonElement | undefined>();
	let contextMenuEl = $state<ContextMenu>();
	let shareIssueModal = $state<ShareIssueModal>();
	let keyboardShortcutsModal = $state<KeyboardShortcutsModal>();

	const userService = inject(USER_SERVICE);
	const userSettings = inject(SETTINGS);
	const { openGeneralSettings, openProjectSettings } = useSettingsModal();
</script>

<div class="sidebar" use:focusable>
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
					<svg
						width="1rem"
						height="1rem"
						viewBox="0 0 16 13"
						fill="none"
						stroke="currentColor"
						xmlns="http://www.w3.org/2000/svg"
					>
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
					<svg
						width="1rem"
						height="1rem"
						viewBox="0 0 16 14"
						fill="none"
						stroke="currentColor"
						xmlns="http://www.w3.org/2000/svg"
					>
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
						width="1.125rem"
						height="1.125rem"
						viewBox="0 0 18 18"
						fill="none"
						stroke="currentColor"
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
								width="1rem"
								height="1rem"
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
		{#if $codegenEnabled}
			<div>
				{#if isCodegenPath()}
					<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
				{/if}
				<Button
					testId={TestId.NavigationCodegenButton}
					kind="outline"
					onclick={() => goto(codegenPath(projectId))}
					width={34}
					class={['btn-square', isCodegenPath() && 'btn-active']}
					tooltip="Codegen"
					tooltipAlign="start"
					{disabled}
				>
					{#snippet custom()}
						<svg
							width="1.375rem"
							height="1.125rem"
							viewBox="0 0 22 18"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
							stroke="currentColor"
						>
							<path
								d="M14.2158 0.0342979C17.5167 -0.254423 20.5442 3.52525 20.9775 8.47668C21.4107 13.4283 19.0852 17.6771 15.7841 17.9659C13.981 18.1235 12.2615 17.0658 10.9999 15.2677C9.73835 17.0658 8.0189 18.1236 6.21576 17.9659C2.91466 17.6771 0.590161 13.4283 1.02337 8.47668C1.4567 3.5254 4.48324 -0.25422 7.78412 0.0342979C9.0257 0.142922 10.1294 0.811718 10.9999 1.86731C11.8705 0.81165 12.9741 0.142926 14.2158 0.0342979Z"
								stroke-width="1.5"
								fill="var(--clr-codegen-bg)"
								stroke="var(--clr-codegen-bg)"
							/>
							<path
								d="M10.691 5.2173C10.7951 4.92757 11.2049 4.92757 11.309 5.2173L11.8782 6.80198C12.0991 7.41684 12.5832 7.90087 13.198 8.12175L14.7827 8.69104C15.0724 8.79513 15.0724 9.20487 14.7827 9.30896L13.198 9.87825C12.5832 10.0991 12.0991 10.5832 11.8782 11.198L11.309 12.7827C11.2049 13.0724 10.7951 13.0724 10.691 12.7827L10.1218 11.198C9.90087 10.5832 9.41684 10.0991 8.80198 9.87825L7.2173 9.30896C6.92757 9.20487 6.92757 8.79513 7.2173 8.69104L8.80198 8.12175C9.41684 7.90087 9.90087 7.41684 10.1218 6.80198L10.691 5.2173Z"
								fill="var(--clr-codegen-star)"
								stroke="var(--clr-codegen-star)"
							/>
							<defs>
								<linearGradient
									id="ai-gradient"
									x1="9.33382"
									y1="1.85727"
									x2="21.498"
									y2="11.1329"
									gradientUnits="userSpaceOnUse"
								>
									<stop stop-color="#9A84F2" />
									<stop offset="0.528846" stop-color="#61B2E1" />
									<stop offset="1" stop-color="#2EDBD2" />
								</linearGradient>
							</defs>
						</svg>
					{/snippet}
				</Button>
			</div>
		{/if}

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
				<Button
					kind="outline"
					onclick={() => {
						openProjectSettings(projectId);
					}}
					width={34}
					class="btn-square"
					tooltipPosition="top"
					tooltipAlign="start"
					tooltip="Project settings"
				>
					{#snippet custom()}
						<svg
							width="0.875rem"
							height="1rem"
							viewBox="0 0 14 16"
							fill="none"
							stroke="currentColor"
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
				openGeneralSettings();
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
	}

	/* OVERRIDE BUTTON STYLES */
	:global(.sidebar .btn-square) {
		& svg {
			opacity: var(--icon-opacity);
			transition: opacity var(--transition-fast);
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

		/* codegen icon */
		--clr-codegen-star: currentColor;
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
		/* codegen icon */
		--clr-codegen-bg: url(#ai-gradient);
		--clr-codegen-star: #ffffff;
	}

	:global(.sidebar .faded-btn) {
		--icon-opacity: 0.4;

		&:hover {
			--icon-opacity: 0.6;
		}
	}
</style>
