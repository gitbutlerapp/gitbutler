<script lang="ts">
	import {
		applyUnityConflictResolutions,
		type UnityConflictDocument,
	} from "$lib/files/unityConflicts";

	import type { UnityConflictChoice, UnityConflictResolution } from "$lib/files/unityConflicts";

	type Props = {
		filePath: string;
		document: UnityConflictDocument;
		onApply: (resolvedContent: string) => Promise<void> | void;
		applying?: boolean;
	};

	const { filePath, document, onApply, applying = false }: Props = $props();

	let resolutions = $state<Record<string, UnityConflictResolution>>({});

	const resolvedCount = $derived(document.blocks.filter((block) => blockResolved(block.id)).length);
	const allResolved = $derived(resolvedCount === document.blocks.length);

	function blockResolved(blockId: string) {
		const resolution = resolutions[blockId];
		if (!resolution) return false;
		if (resolution.choice !== "fields") return true;
		const block = document.blocks.find((candidate) => candidate.id === blockId);
		if (!block || block.fields.length === 0) return false;
		return block.fields.every((field) => resolution.fields?.[field.id] !== undefined);
	}

	function selectChoice(blockId: string, choice: UnityConflictChoice, initialText?: string) {
		resolutions = {
			...resolutions,
			[blockId]: {
				choice,
				manualText:
					choice === "manual"
						? (resolutions[blockId]?.manualText ?? initialText ?? "")
						: resolutions[blockId]?.manualText,
			},
		};
	}

	function selectFieldChoice(
		blockId: string,
		fieldId: string,
		choice: UnityConflictChoice,
		initialText?: string,
	) {
		const blockResolution = resolutions[blockId];
		const previousField = blockResolution?.fields?.[fieldId];
		resolutions = {
			...resolutions,
			[blockId]: {
				choice: "fields",
				fields: {
					...blockResolution?.fields,
					[fieldId]: {
						choice,
						manualText:
							choice === "manual"
								? (previousField?.manualText ?? initialText ?? "")
								: previousField?.manualText,
					},
				},
			},
		};
	}

	function updateManualText(blockId: string, manualText: string) {
		resolutions = {
			...resolutions,
			[blockId]: {
				choice: "manual",
				manualText,
			},
		};
	}

	function updateFieldManualText(blockId: string, fieldId: string, manualText: string) {
		const blockResolution = resolutions[blockId];
		resolutions = {
			...resolutions,
			[blockId]: {
				choice: "fields",
				fields: {
					...blockResolution?.fields,
					[fieldId]: {
						choice: "manual",
						manualText,
					},
				},
			},
		};
	}

	async function handleApply() {
		if (!allResolved) return;

		const resolvedContent = applyUnityConflictResolutions(document, resolutions);
		await onApply(resolvedContent);
	}
</script>

<div class="unity-workbench">
	<div class="unity-workbench__header">
		<div>
			<p class="text-15 text-semibold unity-workbench__title">Unity Scene Resolver</p>
			<p class="text-12 clr-text-2 unity-workbench__subtitle">{filePath}</p>
		</div>
		<div class="unity-workbench__summary">
			<span class="unity-workbench__badge">{resolvedCount}/{document.blocks.length}</span>
			<p class="text-12 clr-text-2">
				{#if allResolved}
					Ready to write the resolved scene back to your workspace.
				{:else}
					Choose a resolution for every conflict block to continue.
				{/if}
			</p>
		</div>
	</div>

	<div class="unity-workbench__blocks">
		{#each document.blocks as block, index (block.id)}
			{@const resolution = resolutions[block.id]}
			<section class="unity-workbench__block">
				<div class="unity-workbench__block-header">
					<div>
						<p class="text-13 text-semibold">{block.label}</p>
						<p class="text-11 clr-text-2">{block.context}</p>
					</div>
					<span class="unity-workbench__badge">Conflict {index + 1}</span>
				</div>

				<div class="unity-workbench__choices">
					<button
						type="button"
						class="choice"
						class:selected={resolution?.choice === "ours"}
						aria-pressed={resolution?.choice === "ours"}
						aria-label={`Use ours for conflict ${index + 1}`}
						onclick={() => selectChoice(block.id, "ours")}
					>
						Use ours
					</button>
					<button
						type="button"
						class="choice"
						class:selected={resolution?.choice === "theirs"}
						aria-pressed={resolution?.choice === "theirs"}
						aria-label={`Use theirs for conflict ${index + 1}`}
						onclick={() => selectChoice(block.id, "theirs")}
					>
						Use theirs
					</button>
					<button
						type="button"
						class="choice"
						class:selected={resolution?.choice === "manual"}
						aria-pressed={resolution?.choice === "manual"}
						aria-label={`Manual edit for conflict ${index + 1}`}
						onclick={() => selectChoice(block.id, "manual", `${block.ours}${block.theirs}`)}
					>
						Manual edit
					</button>
					{#if block.fields.length > 1}
						<button
							type="button"
							class="choice"
							class:selected={resolution?.choice === "fields"}
							aria-pressed={resolution?.choice === "fields"}
							aria-label={`Resolve fields separately for conflict ${index + 1}`}
							onclick={() => selectChoice(block.id, "fields")}
						>
							Choose fields
						</button>
					{/if}
				</div>

				{#if resolution?.choice === "fields"}
					<div class="unity-workbench__fields">
						{#each block.fields as field, fieldIndex (field.id)}
							{@const fieldResolution = resolution.fields?.[field.id]}
							<div class="unity-field">
								<div class="unity-field__header">
									<p class="text-12 text-semibold">{field.label}</p>
									<span class="text-11 clr-text-2">Field {fieldIndex + 1}</span>
								</div>
								<div class="unity-field__choices">
									<button
										type="button"
										class="choice choice--small"
										class:selected={fieldResolution?.choice === "ours"}
										aria-pressed={fieldResolution?.choice === "ours"}
										aria-label={`Use ours for ${field.label}`}
										onclick={() => selectFieldChoice(block.id, field.id, "ours")}
									>
										Use ours
									</button>
									<button
										type="button"
										class="choice choice--small"
										class:selected={fieldResolution?.choice === "theirs"}
										aria-pressed={fieldResolution?.choice === "theirs"}
										aria-label={`Use theirs for ${field.label}`}
										onclick={() => selectFieldChoice(block.id, field.id, "theirs")}
									>
										Use theirs
									</button>
									<button
										type="button"
										class="choice choice--small"
										class:selected={fieldResolution?.choice === "manual"}
										aria-pressed={fieldResolution?.choice === "manual"}
										aria-label={`Manual edit for ${field.label}`}
										onclick={() =>
											selectFieldChoice(
												block.id,
												field.id,
												"manual",
												`${field.ours}${field.theirs}`,
											)}
									>
										Manual
									</button>
								</div>
								<div class="unity-workbench__preview-grid">
									<div class="unity-workbench__preview">
										<p class="text-11 text-semibold unity-workbench__preview-title">Ours</p>
										<pre>{field.ours || "No value"}</pre>
									</div>
									<div class="unity-workbench__preview">
										<p class="text-11 text-semibold unity-workbench__preview-title">Theirs</p>
										<pre>{field.theirs || "No value"}</pre>
									</div>
								</div>
								{#if fieldResolution?.choice === "manual"}
									<label class="unity-workbench__manual text-11">
										<span class="text-semibold">Manual field merge</span>
										<textarea
											aria-label={`Manual resolution for ${field.label}`}
											value={fieldResolution.manualText ?? ""}
											oninput={(event) =>
												updateFieldManualText(
													block.id,
													field.id,
													(event.currentTarget as HTMLTextAreaElement).value,
												)}
										></textarea>
									</label>
								{/if}
							</div>
						{/each}
					</div>
				{:else}
					<div class="unity-workbench__preview-grid">
						<div class="unity-workbench__preview">
							<p class="text-11 text-semibold unity-workbench__preview-title">Ours</p>
							<pre>{block.ours}</pre>
						</div>
						<div class="unity-workbench__preview">
							<p class="text-11 text-semibold unity-workbench__preview-title">Theirs</p>
							<pre>{block.theirs}</pre>
						</div>
					</div>
				{/if}

				{#if resolution?.choice === "manual"}
					<label class="unity-workbench__manual text-11">
						<span class="text-semibold">Manual merge</span>
						<textarea
							aria-label={`Manual resolution for conflict ${index + 1}`}
							value={resolution.manualText ?? ""}
							oninput={(event) =>
								updateManualText(block.id, (event.currentTarget as HTMLTextAreaElement).value)}
						></textarea>
					</label>
				{/if}
			</section>
		{/each}
	</div>

	<div class="unity-workbench__footer">
		<button
			type="button"
			class="unity-workbench__apply"
			disabled={!allResolved || applying}
			onclick={() => void handleApply()}
		>
			Apply to scene
		</button>
	</div>
</div>

<style lang="postcss">
	.unity-workbench {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.unity-workbench__header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		padding: 16px;
		gap: 16px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-l);
		background: linear-gradient(180deg, var(--bg-0), var(--bg-1));
	}

	.unity-workbench__title {
		color: var(--text-1);
	}

	.unity-workbench__subtitle {
		margin-top: 4px;
		word-break: break-all;
	}

	.unity-workbench__summary {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 6px;
		text-align: right;
	}

	.unity-workbench__badge {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-width: 56px;
		padding: 4px 10px;
		border: 1px solid var(--border-2);
		border-radius: 999px;
		background-color: var(--bg-2);
		color: var(--text-1);
		font-weight: 600;
		font-size: 11px;
	}

	.unity-workbench__blocks {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.unity-workbench__block {
		display: flex;
		flex-direction: column;
		padding: 16px;
		gap: 14px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-l);
		background-color: var(--bg-1);
	}

	.unity-workbench__block-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 12px;
	}

	.unity-workbench__choices {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}

	.choice {
		padding: 8px 12px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background-color: var(--bg-2);
		color: var(--text-2);
		font-weight: 600;
		font-size: 12px;
		transition:
			border-color var(--transition-fast),
			background-color var(--transition-fast),
			color var(--transition-fast);

		&:hover {
			border-color: var(--border-1);
			color: var(--text-1);
		}

		&.selected {
			border-color: var(--theme-pop-element);
			background-color: color-mix(in srgb, var(--theme-pop-element) 18%, var(--bg-1));
			color: var(--text-1);
		}
	}

	.choice--small {
		padding: 5px 8px;
		font-size: 11px;
	}

	.unity-workbench__fields {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.unity-field {
		display: flex;
		flex-direction: column;
		padding: 12px;
		gap: 10px;
		border: 1px solid var(--border-3);
		border-radius: var(--radius-m);
		background-color: var(--bg-0);
	}

	.unity-field__header,
	.unity-field__choices {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.unity-field__header {
		justify-content: space-between;
	}

	.unity-field__choices {
		flex-wrap: wrap;
	}

	.unity-workbench__preview-grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 12px;
	}

	.unity-workbench__preview {
		padding: 12px;
		border: 1px solid var(--border-3);
		border-radius: var(--radius-m);
		background-color: var(--bg-0);

		pre {
			margin: 0;
			overflow: auto;
			color: var(--text-1);
			font-size: 12px;
			line-height: 1.5;
			white-space: pre-wrap;
			word-break: break-word;
		}
	}

	.unity-workbench__preview-title {
		margin-bottom: 8px;
		color: var(--text-2);
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}

	.unity-workbench__manual {
		display: flex;
		flex-direction: column;
		gap: 8px;
		color: var(--text-2);

		textarea {
			min-height: 128px;
			padding: 12px;
			border: 1px solid var(--border-2);
			border-radius: var(--radius-m);
			background-color: var(--bg-0);
			color: var(--text-1);
			font: inherit;
			line-height: 1.5;
			resize: vertical;
		}
	}

	.unity-workbench__footer {
		display: flex;
		justify-content: flex-end;
	}

	.unity-workbench__apply {
		padding: 10px 14px;
		border: none;
		border-radius: var(--radius-m);
		background-color: var(--theme-pop-element);
		color: var(--theme-pop-text, white);
		font-weight: 600;
		font-size: 13px;
		transition:
			filter var(--transition-fast),
			opacity var(--transition-fast);

		&:hover:not(:disabled) {
			filter: brightness(1.04);
		}

		&:disabled {
			cursor: not-allowed;
			opacity: 0.55;
		}
	}

	@media (max-width: 900px) {
		.unity-workbench__header {
			flex-direction: column;
		}

		.unity-workbench__summary {
			align-items: flex-start;
			text-align: left;
		}

		.unity-workbench__preview-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
