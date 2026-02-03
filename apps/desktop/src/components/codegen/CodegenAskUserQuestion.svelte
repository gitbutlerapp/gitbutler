<script lang="ts">
	import { AsyncButton, Icon, Textbox } from '@gitbutler/ui';
	import { TestId } from '@gitbutler/ui/utils/testIds';
	import type { AskUserQuestion } from '$lib/codegen/types';

	type Props = {
		questions: AskUserQuestion[];
		answered?: boolean;
		onSubmitAnswers: (answers: Record<string, string>) => Promise<void>;
	};
	const { questions, answered = false, onSubmitAnswers }: Props = $props();

	// Track selected answers for each question (keyed by question text)
	// For single-select: string (the label or 'other')
	// For multi-select: string[] (array of labels, may include 'other')
	let selectedAnswers = $state<Record<string, string | string[]>>({});

	// Track "Other" text input for each question
	let otherText = $state<Record<string, string>>({});

	// Initialize answers only once when questions change, preserving existing selections
	$effect(() => {
		const questionKeys = new Set(questions.map((q) => q.question));

		// Initialize new questions, preserve existing answers
		for (const q of questions) {
			if (!(q.question in selectedAnswers)) {
				selectedAnswers[q.question] = q.multiSelect ? [] : '';
			}
			if (!(q.question in otherText)) {
				otherText[q.question] = '';
			}
		}

		// Clean up answers for removed questions
		for (const key of Object.keys(selectedAnswers)) {
			if (!questionKeys.has(key)) {
				delete selectedAnswers[key];
				delete otherText[key];
			}
		}
	});

	function toggleMultiSelectOption(question: string, label: string) {
		const current = selectedAnswers[question];
		if (Array.isArray(current)) {
			if (current.includes(label)) {
				selectedAnswers[question] = current.filter((l) => l !== label);
			} else {
				selectedAnswers[question] = [...current, label];
			}
		}
	}

	function selectSingleOption(question: string, label: string) {
		selectedAnswers[question] = label;
	}

	function isOptionSelected(question: string, label: string): boolean {
		const current = selectedAnswers[question];
		if (Array.isArray(current)) {
			return current.includes(label);
		}
		return current === label;
	}

	function isOtherSelected(question: string): boolean {
		return isOptionSelected(question, '__other__');
	}

	// Check if all questions have been answered
	const allAnswered = $derived.by(() => {
		for (const q of questions) {
			const answer = selectedAnswers[q.question];
			if (!answer || (Array.isArray(answer) && answer.length === 0)) {
				return false;
			}
			// If "Other" is selected, the text field must have content
			if (isOtherSelected(q.question)) {
				const text = otherText[q.question];
				if (!text || text.trim() === '') {
					return false;
				}
			}
		}
		return true;
	});

	async function handleSubmit() {
		if (!allAnswered) return;

		// Convert answers to the expected format
		const answers: Record<string, string> = {};
		for (const [question, answer] of Object.entries(selectedAnswers)) {
			if (Array.isArray(answer)) {
				// Multi-select: replace '__other__' with the actual text
				const resolvedAnswers = answer.map((a) =>
					a === '__other__' ? (otherText[question] ?? '') : a
				);
				answers[question] = resolvedAnswers.join(', ');
			} else {
				// Single-select: replace '__other__' with the actual text
				answers[question] = answer === '__other__' ? (otherText[question] ?? '') : answer;
			}
		}
		await onSubmitAnswers(answers);
	}
</script>

<div class="ask-user-question" data-testid={TestId.CodegenAskUserQuestion}>
	<div class="ask-user-question__header">
		<Icon name="ai-small" color="var(--clr-text-3)" />
		<span class="text-13 header-text">Claude needs your input</span>
	</div>

	<div class="ask-user-question__questions">
		{#each questions as q}
			<div class="question">
				<div class="question__header">
					<span class="question__badge text-11">{q.header}</span>
					<span class="question__text text-13">{q.question}</span>
				</div>

				<div class="question__options">
					{#each q.options as option}
						<button
							type="button"
							class="option"
							class:selected={isOptionSelected(q.question, option.label)}
							data-testid={TestId.CodegenAskUserQuestion_Option}
							disabled={answered}
							onclick={() => {
								if (q.multiSelect) {
									toggleMultiSelectOption(q.question, option.label);
								} else {
									selectSingleOption(q.question, option.label);
								}
							}}
						>
							<div class="option__indicator">
								{#if q.multiSelect}
									<div class="checkbox" class:checked={isOptionSelected(q.question, option.label)}>
										{#if isOptionSelected(q.question, option.label)}
											<Icon name="tick-small" />
										{/if}
									</div>
								{:else}
									<div class="radio" class:checked={isOptionSelected(q.question, option.label)}>
										{#if isOptionSelected(q.question, option.label)}
											<div class="radio__dot"></div>
										{/if}
									</div>
								{/if}
							</div>
							<div class="option__content">
								<span class="option__label text-13">{option.label}</span>
								<span class="option__description text-12">{option.description}</span>
							</div>
						</button>
					{/each}

					<!-- Other option -->
					<button
						type="button"
						class="option"
						class:selected={isOtherSelected(q.question)}
						disabled={answered}
						onclick={() => {
							if (q.multiSelect) {
								toggleMultiSelectOption(q.question, '__other__');
							} else {
								selectSingleOption(q.question, '__other__');
							}
						}}
					>
						<div class="option__indicator">
							{#if q.multiSelect}
								<div class="checkbox" class:checked={isOtherSelected(q.question)}>
									{#if isOtherSelected(q.question)}
										<Icon name="tick-small" />
									{/if}
								</div>
							{:else}
								<div class="radio" class:checked={isOtherSelected(q.question)}>
									{#if isOtherSelected(q.question)}
										<div class="radio__dot"></div>
									{/if}
								</div>
							{/if}
						</div>
						<div class="option__content">
							<span class="option__label text-13">Other</span>
							<span class="option__description text-12">Provide a custom answer</span>
						</div>
					</button>
				</div>

				<!-- Other text input (shown when Other is selected) -->
				{#if isOtherSelected(q.question)}
					<div class="other-input">
						<Textbox
							placeholder="Enter your answer..."
							bind:value={otherText[q.question]}
							disabled={answered}
							wide
						/>
					</div>
				{/if}

				{#if q.multiSelect}
					<span class="question__hint text-11">Select one or more options</span>
				{/if}
			</div>
		{/each}
	</div>

	<div class="ask-user-question__actions">
		{#if answered}
			<span class="answered-text text-12">
				<Icon name="tick-small" />
				Answered
			</span>
		{:else}
			<AsyncButton
				style="pop"
				disabled={!allAnswered}
				testId={TestId.CodegenAskUserQuestion_SubmitButton}
				action={handleSubmit}
			>
				Submit answers
			</AsyncButton>
		{/if}
	</div>
</div>

<style lang="postcss">
	.ask-user-question {
		display: flex;
		flex-direction: column;
		max-width: var(--message-max-width);
		margin-bottom: 10px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}

	.ask-user-question__header {
		display: flex;
		align-items: center;
		padding: 12px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
	}

	.header-text {
		color: var(--clr-text-1);
		font-weight: 500;
	}

	.ask-user-question__questions {
		display: flex;
		flex-direction: column;
		padding: 12px;
		gap: 16px;
	}

	.question {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.question__header {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.question__badge {
		padding: 2px 6px;
		border-radius: var(--radius-s);
		background-color: var(--clr-bg-3);
		color: var(--clr-text-2);
		font-weight: 500;
	}

	.question__text {
		color: var(--clr-text-1);
		font-weight: 500;
	}

	.question__options {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.question__hint {
		margin-top: 4px;
		color: var(--clr-text-3);
	}

	.option {
		display: flex;
		align-items: flex-start;
		padding: 10px 12px;
		gap: 10px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		text-align: left;
		cursor: pointer;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);

		&:hover:not(:disabled) {
			background-color: var(--clr-bg-2);
		}

		&.selected {
			border-color: var(--clr-theme-pop-element);
			background-color: color-mix(in srgb, var(--clr-theme-pop-element) 8%, transparent);
		}

		&:disabled {
			cursor: default;
			opacity: 0.7;
		}
	}

	.option__indicator {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		margin-top: 2px;
	}

	.checkbox {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 16px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-s);
		background-color: var(--clr-bg-1);
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);

		&.checked {
			border-color: var(--clr-theme-pop-element);
			background-color: var(--clr-theme-pop-element);
			color: var(--clr-theme-pop-on-element);
		}
	}

	.radio {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 16px;
		border: 1px solid var(--clr-border-2);
		border-radius: 50%;
		background-color: var(--clr-bg-1);
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);

		&.checked {
			border-color: var(--clr-theme-pop-element);
		}
	}

	.radio__dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background-color: var(--clr-theme-pop-element);
	}

	.option__content {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.option__label {
		color: var(--clr-text-1);
		font-weight: 500;
	}

	.option__description {
		color: var(--clr-text-2);
	}

	.other-input {
		margin-top: 4px;
		margin-left: 26px;
	}

	.ask-user-question__actions {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		padding: 12px;
		border-top: 1px solid var(--clr-border-2);
	}

	.answered-text {
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--clr-text-2);
	}
</style>
