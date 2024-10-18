<script lang="ts">
	// Define types for the parsed lines and hunk information
	type DiffLine = {
		type: 'added' | 'removed' | 'context' | 'header' | 'hunk';
		content: string;
		lineNumber: number;
		leftLineNumber: number | null; // Use null for lines without line numbers
		rightLineNumber: number | null; // Use null for lines without line numbers
	};

	// Props for the component (with type annotations)
	export let diff: string = '';
	export let diffPath: string = '';
	export let diffSha: string = '';

	// eslint-disable-next-line func-style
	export let onRangeSelect: (range: string, diff_path: string, diff_sha: string) => void = () => {};

	let selectedRange: { startLine: number | null; endLine: number | null } = {
		startLine: null,
		endLine: null
	};

	$: selectedRange.startLine === null; // just to trigger reactivity

	// Handle click event on the gutter line number
	function handleLineNumberClick(line: DiffLine, event: MouseEvent) {
		if (line === null) return;
		if (line.type === 'header' || line.type === 'hunk') return;
		/* dont highlight text when clicking on line number */
		document.getSelection()?.removeAllRanges();

		// Check if Shift key is held to extend selection range
		if (event.shiftKey && selectedRange.startLine !== null) {
			if (line.lineNumber < selectedRange.startLine) {
				selectedRange.endLine = selectedRange.startLine;
				selectedRange.startLine = line.lineNumber;
			} else {
				selectedRange.endLine = line.lineNumber;
			}
			let range = rangeToString(selectedRange);
			onRangeSelect(range, diffPath, diffSha);
		} else {
			// Start a new selection range
			if (selectedRange.startLine === line.lineNumber) {
				selectedRange = { startLine: null, endLine: null };
				onRangeSelect('', '', '');
			} else {
				selectedRange = { startLine: line.lineNumber, endLine: null };
				let range = rangeToString(selectedRange);
				onRangeSelect(range, diffPath, diffSha);
			}
		}
	}

	function rangeToString(range: { startLine: number | null; endLine: number | null }): string {
		let rangeString = '';
		parsedLines.forEach((line) => {
			if (line.lineNumber === range.startLine) {
				if (line.leftLineNumber !== null) {
					rangeString = `L${line.leftLineNumber}`;
				}
				if (line.rightLineNumber !== null) {
					rangeString = `R${line.rightLineNumber}`;
				} else {
					rangeString = ''; // selected a header or something
				}
			}
		});
		if (range.endLine !== null) {
			parsedLines.forEach((line) => {
				if (line.lineNumber === range.endLine) {
					if (line.leftLineNumber !== null) {
						rangeString += `-L${line.leftLineNumber}`;
					} else {
						rangeString += `-R${line.rightLineNumber}`;
					}
				}
			});
		}
		return rangeString;
	}

	// Function to parse the diff string and extract meaningful lines and line numbers
	function parseDiff(diff: string): DiffLine[] {
		const lines = diff.split('\n');
		const parsedLines: DiffLine[] = [];
		let lineNumber: number = 0;
		let leftLineNumber: number = 0;
		let rightLineNumber: number = 0;

		for (const line of lines) {
			lineNumber++;
			// Skip the diff header lines
			if (
				line.startsWith('diff ') ||
				line.startsWith('index ') ||
				line.startsWith('---') ||
				line.startsWith('+++')
			) {
				parsedLines.push({
					type: 'header',
					content: line,
					lineNumber,
					leftLineNumber: null,
					rightLineNumber: null
				});
				continue;
			}

			// If the line starts with '@@', it's a hunk header; extract the starting line number
			if (line.startsWith('@@')) {
				const match = line.match(/@@ -(\d+)(,\d+)? \+(\d+)(,\d+)? @@/);
				if (match) {
					console.log(match);
					leftLineNumber = parseInt(match[1], 10);
					rightLineNumber = parseInt(match[3], 10);
				}
				parsedLines.push({
					type: 'hunk',
					content: line,
					lineNumber,
					leftLineNumber: null,
					rightLineNumber: null
				}); // Display hunk header with no line number
				continue;
			}

			// Determine the type of each line and assign line numbers accordingly
			let type: 'added' | 'removed' | 'context' = 'context';
			let showLeftLineNumber: number | null = null;
			let showRightLineNumber: number | null = null;
			if (line.startsWith('+') && !line.startsWith('+++')) {
				type = 'added';
				rightLineNumber++;
				showRightLineNumber = rightLineNumber - 1;
			} else if (line.startsWith('-') && !line.startsWith('---')) {
				type = 'removed';
				leftLineNumber++;
				showLeftLineNumber = leftLineNumber - 1;
			} else {
				type = 'context';
				rightLineNumber++;
				leftLineNumber++;
				showLeftLineNumber = leftLineNumber - 1;
				showRightLineNumber = rightLineNumber - 1;
			}

			parsedLines.push({
				type,
				content: line,
				lineNumber,
				leftLineNumber: showLeftLineNumber,
				rightLineNumber: showRightLineNumber
			});
		}

		return parsedLines;
	}

	function inRangeClass(lineNumber: number): string {
		if (selectedRange.startLine === null) {
			return '';
		}
		if (selectedRange.endLine === null) {
			if (lineNumber === selectedRange.startLine) {
				return 'inRange startRange endRange';
			}
			return '';
		}
		if (lineNumber >= selectedRange.startLine && lineNumber <= selectedRange.endLine) {
			let rangeClasses = 'inRange';
			if (lineNumber === selectedRange.startLine) {
				rangeClasses += ' startRange';
			}
			if (lineNumber === selectedRange.endLine) {
				rangeClasses += ' endRange';
			}
			return rangeClasses;
		}
		return '';
	}

	// Store parsed lines in a reactive variable
	let parsedLines: DiffLine[] = parseDiff(diff);
</script>

<div class="diff-container">
	<!-- Gutter with line numbers -->
	<div class="gutter">
		{#each parsedLines as line}
			{#if line.type !== 'header'}
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<div
					class={`gutterEntry ${line.type}`}
					on:click={(event) => handleLineNumberClick(line, event)}
				>
					{line.leftLineNumber !== null ? line.leftLineNumber : ' '}
				</div>
			{/if}
		{/each}
	</div>

	<div class="gutter">
		{#each parsedLines as line}
			{#if line.type !== 'header'}
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<div
					class={`gutterEntry ${line.type}`}
					on:click={(event) => handleLineNumberClick(line, event)}
				>
					{line.rightLineNumber !== null ? line.rightLineNumber : ' '}
				</div>
			{/if}
		{/each}
	</div>

	<!-- Content of the diff -->
	<div class="content">
		{#each parsedLines as line}
			{#if line.type !== 'header'}
				<div class={`line ${line.type} ${inRangeClass(line.lineNumber)}`}>
					͏{line.content}
				</div>
			{/if}
		{/each}
	</div>
</div>

<style>
	.diff-container {
		display: flex;
		font-family: monospace;
	}

	.gutter {
		width: 50px;
		background-color: #f7f7f7;
		padding: 0 10px;
		text-align: right;
		color: #999;
		border-right: 1px solid #ddd;
		cursor: pointer;
	}

	.content {
		width: 100%;
		overflow-x: auto;
	}

	.line {
		display: flex;
		white-space: pre; /* Preserve whitespace */
		padding: 4px;
	}

	.line.added {
		background-color: #e6ffed;
	}

	.line.removed {
		background-color: #ffeef0;
	}

	.line.header {
		color: #999;
	}

	.line.hunk {
		color: #556;
		background-color: #cef;
	}

	.gutterEntry {
		padding: 4px;
	}

	.startRange {
		border-top: 2px solid #2076e7;
	}

	.endRange {
		border-bottom: 2px solid #2076e7;
	}

	.inRange {
		border-left: 2px solid #2076e7;
		border-right: 2px solid #2076e7;
		background-color: #e4e4e4;
		color: #000000;
	}

	.inRange.line.added {
		background-color: #9be19b;
		color: #1e4505;
	}

	.inRange.line.removed {
		background-color: #f0bcc2;
		color: rgb(79, 5, 5);
	}
</style>
