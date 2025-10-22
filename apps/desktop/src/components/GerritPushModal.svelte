<script lang="ts" module>
	export type GerritPushModalProps = {
		projectId: string;
		stackId?: string;
		branchName: string;
		multipleBranches: boolean;
		isLastBranchInStack?: boolean;
		isFirstBranchInStack?: boolean;
		onPush: (gerritFlags: import('$lib/stacks/stack').GerritPushFlag[]) => void;
	};

	type PushMode = 'normal' | 'gerrit';
	type GerritFlagType = 'topic' | 'hashtag' | 'wip' | 'ready';
</script>

<script lang="ts">
	import {
		Button,
		Modal,
		RadioButton,
		SectionCard,
		Select,
		SelectItem,
		Textbox
	} from '@gitbutler/ui';
	import type { GerritPushFlag } from '$lib/stacks/stack';

	const {
		projectId: _projectId,
		stackId: _stackId,
		branchName,
		multipleBranches: _multipleBranches,
		isLastBranchInStack: _isLastBranchInStack,
		isFirstBranchInStack: _isFirstBranchInStack,
		onPush
	}: GerritPushModalProps = $props();

	let modal = $state<Modal>();
	let pushMode = $state<PushMode>('normal');
	let selectedGerritFlag = $state<GerritFlagType>('topic');
	let customValue = $state(branchName);

	const gerritFlagOptions = [
		{ label: 'Topic', value: 'topic' as GerritFlagType },
		{ label: 'Hashtag', value: 'hashtag' as GerritFlagType },
		{ label: 'Work in Progress', value: 'wip' as GerritFlagType },
		{ label: 'Ready for Review', value: 'ready' as GerritFlagType }
	];

	// Validate input to allow only alphanumeric characters and dashes
	function validateInput(value: string): string {
		return value.replace(/[^a-zA-Z0-9-]/g, '');
	}

	function handleCustomValueInput(value: string) {
		customValue = validateInput(value);
	}

	function buildGerritFlag(): GerritPushFlag | undefined {
		if (pushMode === 'normal') {
			return undefined;
		}

		switch (selectedGerritFlag) {
			case 'wip':
				return { type: 'wip' };
			case 'ready':
				return { type: 'ready' };
			case 'hashtag':
				return customValue.trim() ? { type: 'hashtag', subject: customValue.trim() } : undefined;
			case 'topic':
				return customValue.trim() ? { type: 'topic', subject: customValue.trim() } : undefined;
		}
	}

	function handlePush() {
		const gerritFlag = buildGerritFlag();
		onPush(gerritFlag ? [gerritFlag] : []);
		modal?.close();
	}

	const needsCustomValue = $derived(
		selectedGerritFlag === 'topic' || selectedGerritFlag === 'hashtag'
	);
	const canPush = $derived(
		pushMode === 'normal' || !needsCustomValue || customValue.trim().length > 0
	);

	export function show() {
		// Reset form state
		pushMode = 'normal';
		selectedGerritFlag = 'topic';
		customValue = branchName;
		modal?.show();
	}

	export function close() {
		modal?.close();
	}
</script>

<Modal bind:this={modal} title="Gerrit Push Options" width="medium" onSubmit={() => handlePush()}>
	<div class="gerrit-push-modal">
		<p class="description text-12 text-body">Choose wether to include additional push options.</p>

		<div class="push-options">
			<!-- Normal Push Section -->
			<SectionCard
				clickable
				orientation="row"
				labelFor="push-mode-normal"
				onclick={() => (pushMode = 'normal')}
				roundedBottom={false}
			>
				{#snippet title()}
					No extra push options
				{/snippet}
				{#snippet caption()}
					GitButler will push without adding any additional options.
				{/snippet}
				{#snippet actions()}
					<RadioButton
						name="pushMode"
						value="normal"
						id="push-mode-normal"
						checked={pushMode === 'normal'}
					/>
				{/snippet}
			</SectionCard>

			<!-- Gerrit Flags Section -->
			<SectionCard
				clickable
				orientation="column"
				labelFor="push-mode-gerrit"
				onclick={() => (pushMode = 'gerrit')}
				roundedTop={false}
			>
				{#snippet title()}
					<div class="gerrit-section-header">
						<span class="gerrit-title">Include Push options</span>
						<RadioButton
							name="pushMode"
							value="gerrit"
							id="push-mode-gerrit"
							checked={pushMode === 'gerrit'}
						/>
					</div>
				{/snippet}
				{#snippet caption()}
					Include additional push options (e.g. the equivalent of <code>%wip</code>,
					<code>%topic=foo</code> etc.).
				{/snippet}
				<!-- eslint-disable-next-line @typescript-eslint/no-unused-vars -->
				{#snippet actions()}
					{#if pushMode === 'gerrit'}
						<div class="gerrit-options">
							<div class="gerrit-options-row">
								<div class="gerrit-flag-select">
									<Select
										value={selectedGerritFlag}
										options={gerritFlagOptions}
										wide
										label="Push option"
										onselect={(value) => {
											selectedGerritFlag = value as GerritFlagType;
											// Reset custom value when changing flag type
											customValue = branchName;
										}}
									>
										{#snippet itemSnippet({ item, highlighted })}
											<SelectItem selected={item.value === selectedGerritFlag} {highlighted}>
												{item.label}
											</SelectItem>
										{/snippet}
									</Select>
								</div>

								{#if needsCustomValue}
									<div class="gerrit-input-field">
										<Textbox
											label="Value"
											bind:value={customValue}
											oninput={handleCustomValueInput}
											placeholder={`Enter ${selectedGerritFlag}`}
											autofocus
											wide
										/>
									</div>
								{/if}
							</div>
						</div>
					{/if}
				{/snippet}
			</SectionCard>
		</div>
	</div>

	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="pop" type="submit" disabled={!canPush}>Push</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.gerrit-push-modal {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.description {
		margin: 0;
		line-height: 1.5;
	}

	.push-options {
		display: flex;
		flex-direction: column;
		gap: 0;
	}

	.gerrit-section-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
	}

	.gerrit-title {
		color: var(--clr-text-1);
		font-weight: 600;
		font-size: 15px;
	}

	.gerrit-options {
		display: flex;
		flex-direction: column;
		margin-top: 12px;
		gap: 12px;
	}

	.gerrit-options-row {
		display: flex;
		align-items: end;
		gap: 12px;
	}

	.gerrit-flag-select,
	.gerrit-input-field {
		flex: 1;
	}
</style>
