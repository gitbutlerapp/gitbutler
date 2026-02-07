<script lang="ts">
	import ConfigurableScrollableContainer from "$components/ConfigurableScrollableContainer.svelte";
	import ReduxResult from "$components/ReduxResult.svelte";
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Tooltip } from "@gitbutler/ui";

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const ircApiService = inject(IRC_API_SERVICE);
	const uiState = inject(UI_STATE);

	const selectedChannel = $derived(uiState.global.channel);
	const channels = $derived(ircApiService.channels());
	const connectionStateQuery = $derived(ircApiService.connectionState());
	const isReady = $derived(connectionStateQuery?.response?.ready ?? false);
	const connectionLabel = $derived(connectionStateQuery?.response?.state ?? "disconnected");

	let joinInput = $state("");

	function joinChannel() {
		const name = joinInput.trim();
		if (!name) return;
		const channel = name.startsWith("#") ? name : `#${name}`;
		joinInput = "";
		selectedChannel.set(channel);
	}
</script>

<div class="channels-wrapper text-13">
	<ReduxResult {projectId} result={channels.result}>
		{#snippet children(channels)}
			{@const sorted = [...channels].sort((a, b) => {
				if (a.name === "*") return -1;
				if (b.name === "*") return 1;
				const aIsChannel = a.name.startsWith("#");
				const bIsChannel = b.name.startsWith("#");
				if (aIsChannel !== bIsChannel) return aIsChannel ? -1 : 1;
				return a.name.localeCompare(b.name);
			})}
			<ConfigurableScrollableContainer>
				{#each sorted as channel}
					{@const unread = channel.unreadCount > 0}
					{@const selected = channel.name === selectedChannel.current}
					{@const isStatus = channel.name === "*"}
					<button
						type="button"
						class="channel"
						class:unread
						class:selected
						onclick={() => selectedChannel.set(channel.name)}
					>
						<span>
							{isStatus ? "ButNet" : channel.name}
						</span>
						{#if isStatus}
							<Tooltip text={connectionLabel}>
								<span class="conn-dot" class:ready={isReady}></span>
							</Tooltip>
						{/if}
					</button>
				{/each}
			</ConfigurableScrollableContainer>
		{/snippet}
	</ReduxResult>
	<div class="join-input-wrapper">
		<input
			type="text"
			class="join-input"
			bind:value={joinInput}
			placeholder="Join #channel"
			onkeydown={(e) => {
				if (e.key === "Enter") joinChannel();
			}}
		/>
	</div>
</div>

<style lang="postcss">
	.channels-wrapper {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		overflow: hidden;
		border-right: 1px solid var(--clr-border-3);
		background-color: var(--clr-bg-2);
	}
	.channel {
		display: flex;
		position: relative;
		align-items: center;
		width: 100%;
		height: 32px;
		padding: 0 0 0 12px;
		overflow: hidden;
		gap: 2px;
		border-top: 1px solid var(--clr-border-3);
		color: var(--clr-text-2);
		text-align: left;
		white-space: nowrap;
		&:first-child {
			border-top: none;
		}
		&.selected {
			background-color: var(--clr-bg-muted);
		}
		& span {
			overflow: hidden;
			text-overflow: ellipsis;
		}
	}

	.selected {
		color: var(--clr-text-1);
	}
	.unread {
		color: var(--clr-text-1);
		font-weight: 600;
	}
	.conn-dot {
		display: inline-block;
		flex-shrink: 0;
		width: 6px;
		height: 6px;
		margin-left: 4px;
		border-radius: 50%;
		background: var(--clr-text-3);

		&.ready {
			background: var(--clr-theme-pop-element);
		}
	}

	.join-input-wrapper {
		display: flex;
		align-items: center;
		padding: 6px;
	}
	.join-input {
		width: 100%;
		height: 36px;
		padding: 8px 6px;
		border-top: 1px solid var(--clr-border-3);
		border: 1px solid var(--clr-border-3);
		border-radius: var(--radius-m);
		outline: none;
		background-color: var(--clr-bg-1);
	}
</style>
