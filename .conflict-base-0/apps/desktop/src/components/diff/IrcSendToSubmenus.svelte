<script lang="ts">
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { inject } from "@gitbutler/core/context";
	import { ContextMenuItem, ContextMenuItemSubmenu, ContextMenuSection } from "@gitbutler/ui";

	type Props = {
		projectId: string;
		disabled?: boolean;
		onSend: (target: string) => void;
		closeMenu: () => void;
	};

	const { projectId, disabled = false, onSend, closeMenu }: Props = $props();

	const ircApiService = inject(IRC_API_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);

	const settingsStore = settingsService.appSettings;
	// Show the submenus whenever the IRC feature flag is on — regardless of
	// whether the connection is currently active.
	const ircFeatureEnabled = $derived($settingsStore?.featureFlags.irc ?? false);

	// Actual connection readiness — used to disable sending while offline.
	const connectionStateQuery = $derived(ircApiService.connectionState());
	const connectionReady = $derived(connectionStateQuery?.response?.ready ?? false);

	const ircChannelsQuery = $derived(ircApiService.channels());
	const ircChannels = $derived((ircChannelsQuery.response ?? []).filter((ch) => ch.name !== "*"));
	const ircNickQuery = $derived(ircApiService.nick());
	const ircNick = $derived(ircNickQuery.response ?? "");
	const ircUsersQuery = $derived(ircApiService.allUsers());

	const sendDisabled = $derived(disabled || !connectionReady);
</script>

{#if ircFeatureEnabled}
	<ReduxResult {projectId} result={ircUsersQuery.result}>
		{#snippet children(ircUsers)}
			{@const filteredUsers = ircUsers.filter((u) => u !== ircNick)}
			{#if ircChannels.length > 0 || filteredUsers.length > 0}
				<ContextMenuSection>
					{#if ircChannels.length > 0}
						<ContextMenuItemSubmenu label="Send to channel">
							{#snippet submenu({ close: closeSubmenu })}
								<ContextMenuSection>
									{#each ircChannels as ch}
										<ContextMenuItem
											label={ch.name}
											disabled={sendDisabled}
											onclick={() => {
												onSend(ch.name);
												closeSubmenu();
												closeMenu();
											}}
										/>
									{/each}
								</ContextMenuSection>
							{/snippet}
						</ContextMenuItemSubmenu>
					{/if}
					{#if filteredUsers.length > 0}
						<ContextMenuItemSubmenu label="Send to user">
							{#snippet submenu({ close: closeSubmenu })}
								<ContextMenuSection>
									{#each filteredUsers as user}
										<ContextMenuItem
											label={user}
											disabled={sendDisabled}
											onclick={() => {
												onSend(user);
												closeSubmenu();
												closeMenu();
											}}
										/>
									{/each}
								</ContextMenuSection>
							{/snippet}
						</ContextMenuItemSubmenu>
					{/if}
				</ContextMenuSection>
			{/if}
		{/snippet}
	</ReduxResult>
{/if}
