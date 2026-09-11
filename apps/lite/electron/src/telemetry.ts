export const posthogHost = "https://eu.i.posthog.com";

export const getPosthogProjectToken = (environment: "development" | "production"): string =>
	environment === "development"
		? "phc_t7VDC9pQELnYep9IiDTxrq2HLseY5wyT7pn0EpHM7rr"
		: "phc_yJx46mXv6kA5KTuM2eEQ6IwNTgl5YW3feKV5gi7mfGG";
