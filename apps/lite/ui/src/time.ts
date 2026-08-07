/** @public */
export const formatRelativeTimeWith =
	(rtf: Intl.RelativeTimeFormat) =>
	(timestamp: number, now = Date.now()): string => {
		const seconds = Math.round((timestamp - now) / 1000);
		const absSeconds = Math.abs(seconds);

		if (absSeconds < 60) return rtf.format(seconds, "seconds");
		if (absSeconds < 60 * 60) return rtf.format(Math.round(seconds / 60), "minutes");
		if (absSeconds < 60 * 60 * 24) return rtf.format(Math.round(seconds / 60 / 60), "hours");
		if (absSeconds < 60 * 60 * 24 * 30)
			return rtf.format(Math.round(seconds / 60 / 60 / 24), "days");
		if (absSeconds < 60 * 60 * 24 * 365)
			return rtf.format(Math.round(seconds / 60 / 60 / 24 / 30), "months");
		return rtf.format(Math.round(seconds / 60 / 60 / 24 / 365), "years");
	};

const stdRelativeTimeFormatter = new Intl.RelativeTimeFormat(undefined, {
	numeric: "always",
	style: "long",
});

export const formatRelativeTime: (timestamp: number, now?: number) => string =
	formatRelativeTimeWith(stdRelativeTimeFormatter);

/** @public */
export const formatDurationWith =
	(df: Intl.DurationFormat) =>
	(ms: number): string => {
		const sign = Math.sign(ms);
		let msRemaining = Math.round(Math.abs(ms));

		const weeks = Math.trunc(msRemaining / 604_800_000);
		msRemaining %= 604_800_000;
		const days = Math.trunc(msRemaining / 86_400_000);
		msRemaining %= 86_400_000;
		const hours = Math.trunc(msRemaining / 3_600_000);
		msRemaining %= 3_600_000;
		const minutes = Math.trunc(msRemaining / 60_000);
		msRemaining %= 60_000;
		const seconds = Math.trunc(msRemaining / 1_000);
		msRemaining %= 1_000;

		return df.format({
			weeks: weeks * sign,
			days: days * sign,
			hours: hours * sign,
			minutes: minutes * sign,
			seconds: seconds * sign,
			milliseconds: msRemaining * sign,
		});
	};

const stdDurationFormatter = new Intl.DurationFormat(undefined, { style: "long" });

export const formatDuration: (ms: number) => string = formatDurationWith(stdDurationFormatter);

const stdAbsoluteTimeFormatter = new Intl.DateTimeFormat(undefined, {
	dateStyle: "medium",
	timeStyle: "short",
});

export const formatAbsoluteTime = (timestamp: number): string =>
	stdAbsoluteTimeFormatter.format(timestamp);
