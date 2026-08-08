import type { ProjectSettingsUpdate } from "@gitbutler/but-sdk";

/**
 * The endpoint takes every field and leaves the null ones alone, so a caller that
 * changes one still has to name the rest. This says which one it means.
 */
export const changing = (fields: Partial<ProjectSettingsUpdate>): ProjectSettingsUpdate => ({
	title: null,
	description: null,
	forcePushProtection: null,
	omitCertificateCheck: null,
	...fields,
});
