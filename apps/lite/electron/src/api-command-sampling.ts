import type { Endpoint } from "./ipc.js";

const API_COMMAND_SAMPLE_RATES: Readonly<Partial<Record<Endpoint, number>>> = {
	treeChangeDiffs: 0.01,
	branchDiff: 0.01,
	changesInWorktree: 0.01,
	commentsList: 0.01,
	commitDetailsWithLineStats: 0.01,
	headInfo: 0.01,
	listProjectsStateless: 0.01,
	listReviews: 0.01,
	workspaceFetchStatus: 0.01,
	workspaceTargetCommits: 0.01,
	branchDetails: 0.1,
	branchList: 0.1,
	getReview: 0.1,
	getReviewMergeStatus: 0.1,
	listCiChecks: 0.1,
	listReviewComments: 0.1,
	listReviewReactions: 0.1,
	listReviewSubmissions: 0.1,
	listReviewThreads: 0.1,
	listReviewTimelineEvents: 0.1,
};

interface FailureLimitConfig {
	bucketSize: number;
	refillIntervalMs: number;
}

interface CaptureDecision {
	occurrenceCount?: number;
	samplingRate: number;
}

interface SamplerOptions {
	failureLimit?: FailureLimitConfig;
	now?: () => number;
	random?: () => number;
}

interface FailureBucket {
	lastRefillAt: number;
	suppressed: number;
	tokens: number;
}

const DEFAULT_FAILURE_LIMIT: FailureLimitConfig = {
	bucketSize: 1,
	refillIntervalMs: 60_000,
};
const MAX_BUCKET_SIZE = 1_000;
const MAX_REFILL_INTERVAL_SECONDS = 3_600;

const apiCommandSampleRate = (command: string): number =>
	API_COMMAND_SAMPLE_RATES[command as Endpoint] ?? 1;

const isIntegerBetween = (value: unknown, maximum: number): value is number =>
	typeof value === "number" && Number.isSafeInteger(value) && value > 0 && value <= maximum;

export const apiCommandFailureLimitConfig = (payload: unknown): FailureLimitConfig => {
	if (payload === null || typeof payload !== "object" || Array.isArray(payload))
		return DEFAULT_FAILURE_LIMIT;

	const { bucketSize, refillIntervalSeconds } = payload as Record<string, unknown>;
	if (
		!isIntegerBetween(bucketSize, MAX_BUCKET_SIZE) ||
		!isIntegerBetween(refillIntervalSeconds, MAX_REFILL_INTERVAL_SECONDS)
	)
		return DEFAULT_FAILURE_LIMIT;

	return { bucketSize, refillIntervalMs: refillIntervalSeconds * 1_000 };
};

export const createApiCommandSampler = ({
	failureLimit = DEFAULT_FAILURE_LIMIT,
	now = () => performance.now(),
	random = Math.random,
}: SamplerOptions = {}) => {
	const buckets = new Map<string, FailureBucket>();

	const sample = (command: string, failure: boolean): CaptureDecision | null => {
		const samplingRate = apiCommandSampleRate(command);
		if (!failure) return random() < samplingRate ? { samplingRate } : null;

		const timestamp = now();
		let bucket = buckets.get(command);
		if (bucket === undefined) {
			bucket = {
				lastRefillAt: timestamp,
				suppressed: 0,
				tokens: failureLimit.bucketSize,
			};
			buckets.set(command, bucket);
		}

		const refills = Math.floor((timestamp - bucket.lastRefillAt) / failureLimit.refillIntervalMs);
		if (refills > 0) {
			bucket.tokens = Math.min(failureLimit.bucketSize, bucket.tokens + refills);
			bucket.lastRefillAt += refills * failureLimit.refillIntervalMs;
		}

		if (bucket.tokens === 0) {
			bucket.suppressed++;
			return null;
		}

		bucket.tokens--;
		const occurrenceCount = bucket.suppressed + 1;
		bucket.suppressed = 0;
		return { occurrenceCount, samplingRate: 1 };
	};

	return {
		sample,
		drainSuppressedFailures: () => {
			const failures = Array.from(buckets, ([command, bucket]) => ({
				command,
				occurrenceCount: bucket.suppressed,
			})).filter(({ occurrenceCount }) => occurrenceCount > 0);
			buckets.clear();
			return failures;
		},
	};
};
