<script lang="ts">
	import { AsyncButton, Badge, Button, Checkbox, RadioButton, Textarea } from "@gitbutler/ui";
	import type { AskUserQuestion } from "$lib/codegen/types";

	type Props = {
		questions: AskUserQuestion[];
		answered?: boolean;
		onSubmitAnswers: (answers: Record<string, string>) => Promise<void>;
		onCancel?: () => void;
	};
	const { questions, answered = false, onSubmitAnswers, onCancel }: Props = $props();

	// Track selected answers for each question (keyed by question index + text)
	// For single-select: string (the label or 'other')
	// For multi-select: string[] (array of labels, may include 'other')
	let selectedAnswers = $state<Record<string, string | string[]>>({});

	// Track "Other" text input for each question
	let otherText = $state<Record<string, string>>({});
	let currentStep = $state(0);

	function getQuestionKey(questionIndex: number, questionText: string): string {
		return `${questionIndex}:${questionText}`;
	}

	// Initialize answers only once when questions change, preserving existing selections
	$effect(() => {
		const questionKeys = new Set(questions.map((q, index) => getQuestionKey(index, q.question)));

		// Initialize new questions, preserve existing answers
		for (const [index, q] of questions.entries()) {
			const key = getQuestionKey(index, q.question);
			if (!(key in selectedAnswers)) {
				selectedAnswers[key] = q.multiSelect ? [] : "";
			}
			if (!(key in otherText)) {
				otherText[key] = "";
			}
		}

		// Clean up answers for removed questions
		for (const key of Object.keys(selectedAnswers)) {
			if (!questionKeys.has(key)) {
				delete selectedAnswers[key];
				delete otherText[key];
			}
		}

		if (currentStep >= questions.length) {
			currentStep = Math.max(0, questions.length - 1);
		}
	});

	const isMultiStep = $derived(questions.length > 1);
	const currentQuestion = $derived(questions[currentStep]);
	const currentQuestionKey = $derived(
		currentQuestion ? getQuestionKey(currentStep, currentQuestion.question) : "",
	);

	function isQuestionAnswered(question: AskUserQuestion, questionIndex: number): boolean {
		const key = getQuestionKey(questionIndex, question.question);
		const answer = selectedAnswers[key];
		if (!answer || (Array.isArray(answer) && answer.length === 0)) {
			return false;
		}
		if (isOtherSelected(key)) {
			const text = otherText[key];
			if (!text || text.trim() === "") {
				return false;
			}
		}
		return true;
	}

	function toggleMultiSelectOption(questionKey: string, label: string) {
		const current = selectedAnswers[questionKey];
		if (Array.isArray(current)) {
			if (current.includes(label)) {
				selectedAnswers[questionKey] = current.filter((l) => l !== label);
			} else {
				selectedAnswers[questionKey] = [...current, label];
			}
		}
	}

	function selectSingleOption(questionKey: string, label: string) {
		selectedAnswers[questionKey] = label;
	}

	function isOptionSelected(questionKey: string, label: string): boolean {
		const current = selectedAnswers[questionKey];
		if (Array.isArray(current)) {
			return current.includes(label);
		}
		return current === label;
	}

	function isOtherSelected(questionKey: string): boolean {
		return isOptionSelected(questionKey, "__other__");
	}

	function activateOption(questionKey: string, label: string) {
		if (currentQuestion?.multiSelect) {
			toggleMultiSelectOption(questionKey, label);
		} else {
			selectSingleOption(questionKey, label);
		}
	}

	function getOptionId(questionIdx: number, optionLabel: string): string {
		return `q${questionIdx}-opt-${optionLabel.replace(/\s+/g, "-").toLowerCase()}`;
	}

	// Check if all questions have been answered
	const allAnswered = $derived.by(() => {
		for (const [index, q] of questions.entries()) {
			if (!isQuestionAnswered(q, index)) {
				return false;
			}
		}
		return true;
	});

	const currentQuestionAnswered = $derived.by(() => {
		if (!currentQuestion) return false;
		return isQuestionAnswered(currentQuestion, currentStep);
	});

	async function handleSubmit() {
		if (!allAnswered) return;

		// Convert answers to the expected format
		const answers: Record<string, string> = {};
		for (const [index, question] of questions.entries()) {
			const key = getQuestionKey(index, question.question);
			const answer = selectedAnswers[key];
			if (!answer) {
				continue;
			}
			if (Array.isArray(answer)) {
				// Multi-select: replace '__other__' with the actual text
				const resolvedAnswers = answer.map((a) => (a === "__other__" ? (otherText[key] ?? "") : a));
				answers[question.question] = resolvedAnswers.join(", ");
			} else {
				// Single-select: replace '__other__' with the actual text
				answers[question.question] = answer === "__other__" ? (otherText[key] ?? "") : answer;
			}
		}
		await onSubmitAnswers(answers);
	}
</script>

<div class="ask-user-question">
	<div class="ask-user-question__questions">
		{#if currentQuestion}
			{#key currentQuestionKey}
				<div class="question">
					<div class="question-header" class:stacked={isMultiStep}>
						{#if isMultiStep}
							<div class="flex gap-4">
								<Badge kind="soft">{currentStep + 1}/{questions.length}</Badge>
								<Badge kind="soft">{currentQuestion.header}</Badge>
							</div>
							<h3 class="text-13 text-bold text-body">{currentQuestion.question}</h3>
						{:else}
							<Badge kind="soft">{currentQuestion.header}</Badge>
							<h3 class="text-13 text-bold text-body">{currentQuestion.question}</h3>
						{/if}
					</div>

					<div class="question-options">
						{#each currentQuestion.options as option (getOptionId(currentStep, option.label))}
							{@const optionId = getOptionId(currentStep, option.label)}
							<label
								for={optionId}
								class="option"
								class:selected={isOptionSelected(currentQuestionKey, option.label)}
								class:disabled={answered}
							>
								<div class="option__indicator">
									{#if currentQuestion.multiSelect}
										<Checkbox
											id={optionId}
											name={`question-${currentStep}`}
											value={option.label}
											small
											disabled={answered}
											checked={isOptionSelected(currentQuestionKey, option.label)}
											onchange={() => {
												if (answered) return;
												activateOption(currentQuestionKey, option.label);
											}}
										/>
									{:else}
										<RadioButton
											id={optionId}
											name={`question-${currentStep}`}
											value={option.label}
											small
											disabled={answered}
											checked={isOptionSelected(currentQuestionKey, option.label)}
											onchange={() => {
												if (answered) return;
												activateOption(currentQuestionKey, option.label);
											}}
										/>
									{/if}
								</div>

								<div class="option__content">
									<span class="option__label text-13 text-body">{option.label}</span>
									<span class="option__description text-12 text-body">{option.description}</span>
								</div>
							</label>
						{/each}

						<!-- Other option -->
						<label
							for={getOptionId(currentStep, "other")}
							class="option"
							class:selected={isOtherSelected(currentQuestionKey)}
							class:disabled={answered}
						>
							<div class="option__indicator">
								{#if currentQuestion.multiSelect}
									<Checkbox
										id={getOptionId(currentStep, "other")}
										name={`question-${currentStep}`}
										value="__other__"
										small
										disabled={answered}
										checked={isOtherSelected(currentQuestionKey)}
										onchange={() => {
											if (answered) return;
											activateOption(currentQuestionKey, "__other__");
										}}
									/>
								{:else}
									<RadioButton
										id={getOptionId(currentStep, "other")}
										name={`question-${currentStep}`}
										value="__other__"
										small
										disabled={answered}
										checked={isOtherSelected(currentQuestionKey)}
										onchange={() => {
											if (answered) return;
											activateOption(currentQuestionKey, "__other__");
										}}
									/>
								{/if}
							</div>
							<div class="option__content">
								<Textarea
									flex="1"
									unstyled
									placeholder="Need something else? Describe it here..."
									bind:value={otherText[currentQuestionKey]}
									disabled={answered}
									onfocus={() => {
										if (answered) return;
										activateOption(currentQuestionKey, "__other__");
									}}
								/>
							</div>
						</label>
					</div>

					{#if currentQuestion.multiSelect}
						<span class="question__hint text-11">Select one or more options</span>
					{/if}
				</div>
			{/key}
		{/if}
	</div>

	<div class="ask-user-question__actions">
		{#if !isMultiStep}
			<div class="flex flex-1">
				<Button
					kind="outline"
					style="danger"
					disabled={!onCancel}
					icon="cross-small"
					onclick={() => {
						onCancel?.();
					}}>Discard and exit</Button
				>
			</div>

			<AsyncButton style="pop" icon="arrow-up" disabled={!allAnswered} action={handleSubmit}>
				Submit answers
			</AsyncButton>
		{:else}
			<div class="flex flex-1">
				<Button
					kind="outline"
					style="danger"
					disabled={!onCancel}
					icon="cross-small"
					onclick={() => {
						onCancel?.();
					}}>Discard and exit</Button
				>
			</div>

			<div class="flex gap-6">
				{#if currentStep > 0}
					<Button
						kind="outline"
						reversedDirection
						icon="arrow-left"
						onclick={() => {
							currentStep = Math.max(0, currentStep - 1);
						}}
					/>
				{/if}

				{#if currentStep < questions.length - 1}
					<Button
						icon="arrow-right"
						disabled={!currentQuestionAnswered}
						onclick={() => {
							if (currentQuestionAnswered) {
								currentStep = Math.min(questions.length - 1, currentStep + 1);
							}
						}}>Next question</Button
					>
				{:else}
					<AsyncButton style="pop" disabled={!allAnswered} action={handleSubmit}>
						Submit answers
					</AsyncButton>
				{/if}
			</div>
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

	.question-header {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 6px;
	}

	.question-options {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.question__hint {
		margin-top: 4px;
		color: var(--clr-text-3);
	}

	.option {
		display: flex;
		align-items: flex-start;
		padding: 10px 14px;
		gap: 12px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: transparent;
		cursor: pointer;
		transition: background-color var(--transition-fast);
		user-select: none;

		&:last-child {
			border-bottom: none;
		}

		&:hover:not(.disabled) {
			background-color: var(--hover-bg-1);
		}

		&.disabled {
			cursor: not-allowed;
			opacity: 0.6;
		}
	}

	.option__indicator {
		display: flex;
		flex-shrink: 0;
		margin-top: 4px;
	}

	.option__content {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 2px;
	}

	.option__label {
		color: var(--clr-text-1);
		font-weight: 500;
	}

	.option__description {
		color: var(--clr-text-2);
	}

	.ask-user-question__actions {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		padding: 12px;
		border-top: 1px solid var(--clr-border-2);
	}
</style>
