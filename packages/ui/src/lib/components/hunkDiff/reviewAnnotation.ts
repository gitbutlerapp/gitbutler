export type ReviewAnnotationSeverity = "critical" | "major" | "minor" | "info";

export type ReviewAnnotation = {
	id: string;
	path: string;
	oldLine?: number;
	newLine?: number;
	severity: ReviewAnnotationSeverity;
	title: string;
	body: string;
	suggestedPatch?: string;
};
