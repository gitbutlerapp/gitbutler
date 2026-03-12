/* eslint-disable no-console */
import {
	longRunningCancelTsfn,
	LongRunningEventKind,
	longRunningStartTsfn,
} from "./generated/index.js";

function isNumber(something: unknown): something is number {
	return typeof something === "number";
}

/**
 * So what's the deal here?
 *
 * We're testing running a long-running process being triggered by JS but not blocking it.
 *
 * The process emits back events that we can read and react to.
 *
 * It can be stopped and handled gracefully.
 */
async function runLongRunning({
	durationMs,
	signal,
	onProgress,
}: {
	durationMs: number;
	signal?: AbortSignal;
	onProgress?: (step: number) => void;
}) {
	return await new Promise<{ lastStep: number }>((resolve, reject) => {
		let taskId = 0;
		let lastStep = 0;

		taskId = longRunningStartTsfn(durationMs, (err, event) => {
			if (err) {
				reject(err);
				return;
			}

			if (isNumber(event.step)) {
				lastStep = event.step;
			}

			if (event.kind === LongRunningEventKind.Progress && isNumber(event.step)) {
				onProgress?.(event.step);
				return;
			}

			if (event.kind === LongRunningEventKind.Done) {
				resolve({ lastStep });
				return;
			}

			if (event.kind === LongRunningEventKind.Cancelled) {
				reject(new Error("yep. interrupted"));
				return;
			}

			if (event.kind === LongRunningEventKind.Error) {
				reject(new Error(event.message ?? "unknown error"));
			}
		});

		console.log("start long running process, but don't block");

		if (signal) {
			console.log("there is a signal");
			if (signal.aborted) {
				console.log("signal is aborted");
				longRunningCancelTsfn(taskId);
			} else {
				signal.addEventListener(
					"abort",
					() => {
						console.log("signal abort event triggered");
						longRunningCancelTsfn(taskId);
					},
					{ once: true },
				);
			}
		}
	});
}
async function main() {
	const abortController = new AbortController();
	setTimeout(() => {
		// Abort after a second
		console.log("after waiting a second, interrupt.");
		abortController.abort();
	}, 1000);

	const result = await runLongRunning({
		durationMs: 5000,
		signal: abortController.signal,
		onProgress: (step: number) => console.log(`step ${step}`),
	}).catch((e) => `probably interrupt error: ${e}`);

	console.log("\nresult");
	console.log(result);
}

main();
