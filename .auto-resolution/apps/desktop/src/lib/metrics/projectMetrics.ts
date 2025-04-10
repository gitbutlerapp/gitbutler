import { getEphemeralStorageItem, setEphemeralStorageItem } from '@gitbutler/shared/persisted';

export type MetricsReport = {
	[key: string]: ProjectMetric | undefined;
};

type ProjectMetric = {
	value: number;
	minValue: number;
	maxValue: number;
};

const REPORT_KEY = 'metricsReport';
const STORAGE_EXPIRY_MINUTES = 24 * 60;

/**
 * Tracks arbitrary metrics and keeps track of min/max values. Please note that
 * reporting these numbers to the back end is delegated to the MetricsReporter
 * component.
 */
export class ProjectMetrics {
	private reports: Record<string, MetricsReport> = {};

	project(id: string) {
		let project = this.reports[id];
		if (project) return project;
		project = {};
		this.reports[id] = project;
		return project;
	}

	get reportKey() {
		return `${REPORT_KEY}`;
	}

	setMetric(projectId: string, key: string, value: number) {
		// Guard against upstream bugs feeding bad values.
		if (typeof value !== 'number' || !Number.isFinite(value) || Number.isNaN(value)) {
			console.warn(`Ignoring ${key} metric, bad value: ${value}`);
			return;
		}
		const oldEntry = this.project(projectId)[key];
		if (oldEntry) {
			const { maxValue, minValue } = oldEntry;
			this.project(projectId)[key] = {
				value,
				maxValue: Math.max(value, maxValue),
				minValue: Math.min(value, minValue)
			};
		} else {
			this.project(projectId)[key] = {
				value,
				maxValue: value,
				minValue: value
			};
		}
	}

	saveToLocalStorage() {
		setEphemeralStorageItem(this.reportKey, this.reports, STORAGE_EXPIRY_MINUTES);
	}

	loadFromLocalStorage() {
		const reports = getEphemeralStorageItem(this.reportKey) as
			| Record<string, MetricsReport>
			| undefined;
		if (reports) {
			this.reports = reports;
		}
	}

	getReport(projectId: string): MetricsReport {
		// Return a copy since we keep mutating the metrics object,
		// and a report is specific to a point in time.
		return structuredClone(this.project(projectId));
	}

	resetMetric(projectId: string, key: string) {
		delete this.project(projectId)[key];
	}
}
