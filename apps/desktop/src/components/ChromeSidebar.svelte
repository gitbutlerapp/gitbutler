<script lang="ts">
	import { goto } from "$app/navigation";
	import ProfileButton from "$components/ProfileButton.svelte";
	import ShareIssueModal from "$components/ShareIssueModal.svelte";
	import { SETTINGS_SERVICE } from "$lib/config/appSettingsV2";
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import {
		branchesPath,
		isBranchesPath,
		isWorkspacePath,
		historyPath,
		isHistoryPath,
		workspacePath,
	} from "$lib/routes/routes.svelte";
	import { useSettingsModal } from "$lib/settings/settingsModal.svelte";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		Badge,
		Button,
		ContextMenu,
		ContextMenuItem,
		ContextMenuSection,
		TestId,
	} from "@gitbutler/ui";
	import { focusable } from "@gitbutler/ui/focus/focusable";

	import { slide } from "svelte/transition";

	const { projectId, disabled = false }: { projectId: string; disabled?: boolean } = $props();

	let contextTriggerButton = $state<HTMLButtonElement | undefined>();
	let contextMenuEl = $state<ContextMenu>();
	let shareIssueModal = $state<ShareIssueModal>();

	const userSettings = inject(SETTINGS);
	const uiState = inject(UI_STATE);
	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = settingsService.appSettings;
	const ircEnabled = $derived($settingsStore?.irc.connection.enabled ?? false);
	const ircApiService = inject(IRC_API_SERVICE);
	const ircChannelsQuery = $derived(ircEnabled ? ircApiService.channels() : undefined);
	const ircUnreadChannels = $derived(
		(ircChannelsQuery?.response ?? []).filter((ch) => ch.unreadCount > 0).length,
	);
	const { openGeneralSettings, openProjectSettings } = useSettingsModal();
</script>

<div class="sidebar" use:focusable>
	<div class="top">
		<div>
			{#if isWorkspacePath()}
				<div class="active-page-indicator" in:slide={{ axis: "x", duration: 150 }}></div>
			{/if}
			<Button
				testId={TestId.NavigationWorkspaceButton}
				kind="outline"
				onclick={() => goto(workspacePath(projectId))}
				width={34}
				hotkey="⌘1"
				class={["btn-square", isWorkspacePath() && "btn-active"]}
				tooltip="Workspace"
				{disabled}
			>
				{#snippet custom()}
					<svg
						width="1.063rem"
						height="0.813rem"
						viewBox="0 0 17 13"
						fill="none"
						stroke="currentColor"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path
							d="M2.2135 12.7501L3.7135 8.25006M4.7135 4.88403L3.7135 8.25006M14.2135 12.7501L12.7135 8.25006M11.7135 4.75L12.7135 8.25006M12.7135 8.25006H3.7135"
							stroke="var(--clr-workspace-legs)"
							stroke-width="1.5"
						/>
						<path
							d="M1.2135 4.75H15.2135L13.2135 0.75H3.2135L1.2135 4.75Z"
							stroke="var(--clr-workspace-top)"
							fill="var(--clr-workspace-top)"
							stroke-width="1.5"
						/>
					</svg>
				{/snippet}
			</Button>
		</div>
		<div>
			{#if isBranchesPath()}
				<div class="active-page-indicator" in:slide={{ axis: "x", duration: 150 }}></div>
			{/if}
			<Button
				testId={TestId.NavigationBranchesButton}
				kind="outline"
				onclick={() => goto(branchesPath(projectId))}
				width={34}
				class={["btn-square", isBranchesPath() && "btn-active"]}
				hotkey="⌘2"
				tooltip="Branches"
				{disabled}
			>
				{#snippet custom()}
					<svg
						width="1.063rem"
						height="0.938rem"
						viewBox="0 0 17 15"
						fill="none"
						stroke="currentColor"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path d="M5.75 3.25H10.75" stroke="var(--clr-branches)" stroke-width="1.5" />
						<path
							d="M3.75 5.25L5.06623 9.19868C5.47457 10.4237 6.62099 11.25 7.91228 11.25L10.75 11.25"
							stroke="var(--clr-branches)"
							stroke-width="1.5"
						/>
						<rect
							x="15.75"
							y="0.75"
							width="5"
							height="5"
							rx="2.5"
							transform="rotate(90 15.75 0.75)"
							fill="var(--clr-branches)"
							stroke="var(--clr-branches)"
							stroke-width="1.5"
						/>
						<rect
							x="5.75"
							y="0.75"
							width="5"
							height="5"
							rx="2.5"
							transform="rotate(90 5.75 0.75)"
							fill="var(--clr-branches)"
							stroke="var(--clr-branches)"
							stroke-width="1.5"
						/>
						<rect
							x="15.75"
							y="8.75"
							width="5"
							height="5"
							rx="2.5"
							transform="rotate(90 15.75 8.75)"
							fill="var(--clr-branches)"
							stroke="var(--clr-branches)"
							stroke-width="1.5"
						/>
					</svg>
				{/snippet}
			</Button>
		</div>
		<div>
			{#if isHistoryPath()}
				<div class="active-page-indicator" in:slide={{ axis: "x", duration: 150 }}></div>
			{/if}
			<Button
				kind="outline"
				onclick={() => goto(historyPath(projectId))}
				width={34}
				class={["btn-square", isHistoryPath() && "btn-active"]}
				hotkey="⌘3"
				tooltip="Operations history"
				{disabled}
			>
				{#snippet custom()}
					<svg
						width="1.188rem"
						height="1.125rem"
						viewBox="0 0 19 18"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						{#if isHistoryPath()}
							<circle cx="9.82397" cy="8.75" r="8" fill="#FBDB79" />
						{/if}
						<path
							d="M9.10022 4.20642V9.29948H14.1933"
							stroke="var(--clr-history-arrows)"
							stroke-width="1.5"
						/>
						<path
							d="M2.40555 5.75C3.59233 2.81817 6.46666 0.75 9.82403 0.75C14.2423 0.75 17.824 4.33172 17.824 8.75C17.824 13.1683 14.2423 16.75 9.82403 16.75C6.27881 16.75 3.2722 14.4439 2.22241 11.25"
							stroke="var(--clr-history-outline)"
							stroke-width="1.5"
						/>
						<path
							d="M0 1.78357L6.62924 7.08009L0.666363 7.74645L0 1.78357Z"
							fill="var(--clr-history-outline)"
						/>
					</svg>
				{/snippet}
			</Button>
		</div>
		{#if ircEnabled}
			{@const ircChatOpen = uiState.global.ircChatOpen}
			<div class="irc-btn-wrap">
				{#if ircChatOpen.current}
					<div class="active-page-indicator" in:slide={{ axis: "x", duration: 150 }}></div>
				{/if}
				<Button
					kind="outline"
					onclick={() => ircChatOpen.set(!ircChatOpen.current)}
					icon="chat"
					width={34}
					class={["btn-square", ircChatOpen.current ? "btn-active" : undefined]}
					tooltip="IRC Chat"
					{disabled}
				/>
				{#if ircUnreadChannels > 0 && !ircChatOpen.current}
					<div class="unread-badge">
						<Badge style="pop" size="icon">{ircUnreadChannels}</Badge>
					</div>
				{/if}
			</div>
		{/if}
	</div>
	<div class="bottom">
		<div class="bottom__primary-actions">
			<div>
				<Button
					testId={TestId.ChromeSideBarProjectSettingsButton}
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
								d="M5.8125 1.01783C6.43123 0.6608 7.19383 0.660645 7.8125 1.01783L11.874 3.36354C12.4928 3.7208 12.874 4.38143 12.874 5.09596V9.78639C12.8739 10.5008 12.4928 11.1606 11.874 11.5178L7.8125 13.8635C7.19378 14.2208 6.43126 14.2207 5.8125 13.8635L1.75 11.5178C1.13136 11.1606 0.750083 10.5008 0.75 9.78639V5.09596C0.75 4.38143 1.13121 3.7208 1.75 3.36354L5.8125 1.01783ZM7.8125 5.01783C7.19383 4.66065 6.43123 4.6608 5.8125 5.01783L5.21387 5.36354C4.59518 5.72083 4.21387 6.3815 4.21387 7.09596V7.78639C4.21395 8.5007 4.59533 9.16056 5.21387 9.51783L5.8125 9.86354C6.43126 10.2207 7.19378 10.2208 7.8125 9.86354L8.41016 9.51783C9.02884 9.16059 9.41007 8.5008 9.41016 7.78639V7.09596C9.41016 6.38143 9.02896 5.7208 8.41016 5.36354L7.8125 5.01783Z"
								stroke="var(--clr-settings-bg)"
								fill="var(--clr-settings-bg)"
								stroke-width="1.5"
							/>
						</svg>
					{/snippet}
				</Button>
			</div>

			<ProfileButton />
		</div>
		<div class="bottom__ghost-actions">
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
					theme: "dark",
				}));
				contextMenuEl?.close();
			}}
		/>
		<ContextMenuItem
			label="Light"
			onclick={async () => {
				userSettings.update((s) => ({
					...s,
					theme: "light",
				}));
				contextMenuEl?.close();
			}}
		/>
		<ContextMenuItem
			label="System"
			onclick={async () => {
				userSettings.update((s) => ({
					...s,
					theme: "system",
				}));
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
</ContextMenu>

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
		position: relative;
		flex-direction: column;
		gap: 8px;
	}
	.bottom__ghost-actions {
		display: flex;
		position: relative;
		flex-direction: column;
		gap: 2px;
	}

	.irc-btn-wrap {
		position: relative;
	}

	.unread-badge {
		position: absolute;
		top: -4px;
		right: -4px;
		pointer-events: none;
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
		--clr-history-outline: currentColor;
		--clr-history-arrows: currentColor;
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
		--clr-history-arrows: #000;
		--clr-history-outline: #e98959;
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

		&:not(:disabled):hover {
			--icon-opacity: 0.6;
		}
	}
</style>
