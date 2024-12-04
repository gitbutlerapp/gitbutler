import { getEphemeralStorageItem, setEphemeralStorageItem } from '@gitbutler/shared/persisted';

export type MetricsReport = {
	[key: string]: ProjectMetric | undefined;
};

type ProjectMetric = {
	value: number;
	minValue: number;
	maxValue: number;
};

const REPORT_PREFIX = 'lastReport';
const STORAGE_EXPIRY_MINUTES = 24 * 60;

/**
 * Tracks arbitrary metrics and keeps track of min/max values. Please note that
 * reporting these numbers to the back end is delegated to the MetricsReporter
 * component.
 */
export class ProjectMetrics {
	// Storing the last known values so we don't report same metrics twice
	private reportKey = `${REPORT_PREFIX}-${this.projectId}`;

	private report: MetricsReport = {};

	constructor(readonly projectId: string) {}

	setMetric(key: string, value: number) {
		// Guard against upstream bugs feeding bad values.
		if (typeof value !== 'number' || !Number.isFinite(value) || Number.isNaN(value)) {
			console.warn(`Ignoring ${key} metric, bad value: ${value}`);
			return;
		}
		const oldEntry = this.report[key];
		if (oldEntry) {
			const { maxValue, minValue } = oldEntry;
			this.report[key] = {
				value,
				maxValue: Math.max(value, maxValue),
				minValue: Math.min(value, minValue)
			};
		} else {
			this.report[key] = {
				value,
				maxValue: value,
				minValue: value
			};
		}
	}

	saveToLocalStorage() {
		setEphemeralStorageItem(this.reportKey, this.report, STORAGE_EXPIRY_MINUTES);
	}

	loadFromLocalStorage() {
		const report = getEphemeralStorageItem(this.reportKey) as MetricsReport | undefined;
		if (report) {
			this.report = report;
		}
	}

	getReport(): MetricsReport {
		// Return a copy since we keep mutating the metrics object,
		// and a report is specific to a point in time.
		return structuredClone(this.report);
	}

	resetMetric(key: string) {
		delete this.report[key];
	}
}
